//! 线上协议（OpenAI Chat Completions，LLM.md v3.0）：请求装配 / SSE chunk
//! 解析 / 用量映射，纯函数 + 流式累积器。**唯一线路**——非流式 `complete`
//! 也走这条管道聚合（无第二形状）。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use session_store::{ContentPart, Message};

use kernel_agent::{estimate_tokens, LlmRequest, LlmResponse, LlmUsage, ToolCallReq};
use plugin_api::{CallId, ToolSpec};

use super::dialect::Dialect;

/// 模型能力上限（批 B，AppLlm 按 settings 钉死面解析后传入；profile 级
/// 拿不到 = 全 None，行为与 v2.75 一致——不发明派生限额）。
#[derive(Debug, Clone, Copy, Default)]
pub struct WireCaps {
    /// 用户钉死最大输出（ProviderModel.max_output；仅用户值，目录值不发）。
    pub max_output: Option<u64>,
    /// 生效上下文窗口（模型行 context_window ?? 档位 window_tokens）。
    pub ctx_window: Option<u64>,
    /// Q8 工具结果预算（**字节**，None = 不限制）。由组装点从
    /// `settings.context.tool_result_budget_kib` 换算带下来——KiB→字节的算式在
    /// `settings/context.rs::tool_result_budget_bytes` 里只有一份，wire 不再
    /// 自己乘 1024（同一读数多个出处必漂，§11）。
    pub tool_result_budget_bytes: Option<usize>,
}

/// Q8：超预算的工具输出按字节截断，并追加一句**可续读**提示。三点是刻意的：
/// ①只动发给模型的这一份，会话日志仍是全量真相源（回捞 / 导出 / 崩溃重放读它）；
/// ②切点回退到 UTF-8 字符边界——中文/emoji 按字节硬切会当场 panic，这不是假设中的
///   风险，`fs_read` 读一个中文文件就可能让边界落在多字节序列中间；
/// ③`None`（预算 0）时逐字节原样返回 = 与改动前完全一致，这条等价关系由测试钉住。
fn clip_tool_output(content: String, budget: Option<usize>) -> String {
    let Some(limit) = budget else {
        return content;
    };
    if content.len() <= limit {
        return content;
    }
    let mut cut = limit;
    while cut > 0 && !content.is_char_boundary(cut) {
        cut -= 1;
    }
    let total = content.len();
    format!(
        "{}\n\n【工具结果已按「工具结果预算」截断：原输出 {total} 字节，此处只有前 {cut} 字节。\
         完整内容仍在本会话日志里；需要后面的部分请用同一工具的 offset / max_bytes 参数续取\
         （该预算在 设置 → 上下文 可调，0 = 不限制）。】",
        &content[..cut]
    )
}

/// 预检钳制下限：估用已吃掉空间时不钳到负数，留给上下文溢出语义接管。
const MIN_MAX_TOKENS: u64 = 1024;
/// 预检钳制的安全余量（供应商稿 §6）。
const CLAMP_HEADROOM: i64 = 1000;

/// 请求装配（LLM.md「请求装配」节）：中立投影直转；`stream_options` 只在
/// 流式携带；**max_tokens 只在用户钉死值在场时发送 + 预检钳制**
/// `min(钉死, ctx − 估用 − 1000)`（供应商稿 §6 窄修订，D5 裁定）；推理
/// 档位按方言写字段（`dialect.patch`，映射不了不发）。
/// `assets`（批 D）：带附件消息的 asset 字节读取器（AppLlm 组装点注入；
/// None + 请求带附件 = Fatal——没处读字节的附件不能静默丢）。
pub fn build_payload(
    model: &str,
    req: &LlmRequest,
    stream: bool,
    dialect: Dialect,
    caps: &WireCaps,
    assets: Option<&crate::assets::AssetStore>,
) -> Result<Value, String> {
    let mut messages: Vec<Value> = Vec::with_capacity(req.messages.len() + 1);
    if let Some(sys) = &req.system {
        messages.push(json!({ "role": "system", "content": sys }));
    }
    for m in &req.messages {
        messages.push(message_value(m, assets, caps.tool_result_budget_bytes)?);
    }
    let mut payload = json!({ "model": model, "messages": messages, "stream": stream });
    if stream {
        payload["stream_options"] = json!({ "include_usage": true });
    }
    if !req.tools.is_empty() {
        payload["tools"] = Value::Array(
            req.tools
                .iter()
                .map(|t: &ToolSpec| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.schema,
                        },
                    })
                })
                .collect(),
        );
    }
    if let Some(t) = req.params.get("temperature").and_then(Value::as_f64) {
        payload["temperature"] = json!(t);
    }
    // 推理档位（批 B）：params["thinking_level"]（会话旁车 clamp 后落进来；
    // 辅助降档同字段）→ 方言字段。映射不了 = 不发（宁缺勿假，记一行日志）。
    if let Some(level) = req.params.get("thinking_level").and_then(Value::as_str) {
        match dialect.patch(level) {
            Some(fields) => {
                let obj = payload.as_object_mut().expect("payload object");
                for (k, v) in fields {
                    obj.insert(k.to_string(), v);
                }
            }
            None => {
                tracing::debug!(dialect = ?dialect, level, "推理档位无方言写法，不发送");
            }
        }
    }
    // 输出上限（D5 窄修订）：只有用户钉死值才发送；发送前预检钳制
    if let Some(pinned) = caps.max_output {
        let est = estimate_tokens(&req.messages) as i64;
        let max_tokens = match caps.ctx_window {
            Some(ctx) => (pinned as i64).min(ctx as i64 - est - CLAMP_HEADROOM),
            None => pinned as i64,
        };
        payload["max_tokens"] = json!(max_tokens.max(MIN_MAX_TOKENS as i64) as u64);
    }
    Ok(payload)
}

/// 发送前能力门控（供应商稿 §7 第二道防线；第一道在批 D 前端上传时）：
/// 请求带图片 part 且生效模型图像输入三态解析为 `Some(false)` → 拒发带
/// 指引。**未知（None）放行**——目录没有的能力不发明（宁缺勿假）。
pub fn check_modalities(req: &LlmRequest, image_allowed: Option<bool>) -> Result<(), String> {
    let has_image = req.messages.iter().any(|m| {
        matches!(m, Message::User { parts, .. } if parts
            .iter()
            .any(|p| matches!(p, ContentPart::Image { .. })))
    });
    if has_image && image_allowed == Some(false) {
        return Err("当前模型不支持图片输入：请在 设置 → 模型 里换用视觉模型，\
             或去掉图片附件后重发"
            .into());
    }
    Ok(())
}

/// 工具结果图片投影（批 E，多模态稿 §8.1；裁定 D4 按推荐落档 = 投影）。
/// 工具 output 里的 `asset`/`assets`（mime `image/*`）从 tool 消息挪出——
/// tool 消息只吃文本，图片随消息列尾部合成的一条 user 消息发送（agent 循环
/// 里工具结果就在尾部，「下一条 user 消息」多数时候不存在，合成一条只带
/// 图片的 user 消息 = ZCode 媒体投影同思路；设计稿「无下一条 user 保持
/// 占位」的形态记档偏离——占位会让模型永远看不到图，主流程不可用）。
///
/// 三条纪律：
/// * **工具消息改写是位置无关的纯函数**——note 文本跨请求逐字节稳定
///   （M1 前缀缓存纪律）；历史轮里已有后续 user 消息的工具图片**不重发**
///   （每请求重发 = token 永久膨胀；模型在工具结果当轮已看过一次）。
/// * 模型不支持图片（`Some(false)`）→ 不投影不报错：引用号留原地 + note
///   指引（工具文本输出照常可用；整轮 Fatal 不成比例）。
/// * 调用点在 `AppLlm::prepare`（image_allowed 已在场）——maxOutput 钳制
///   估算吃的是投影后消息，设计稿记档的估算缺口在调用点闭合。
pub fn project_tool_assets(messages: Vec<Message>, image_allowed: Option<bool>) -> Vec<Message> {
    let has_images = messages.iter().any(|m| match m {
        Message::ToolResult { output, error, .. } => {
            error.is_none() && !tool_image_assets(output).is_empty()
        }
        _ => false,
    });
    if !has_images {
        return messages;
    }
    let mut tail_assets: Vec<session_store::AssetRef> = Vec::new();
    let mut user_seen = false; // 从尾往前扫：更晚的 user 消息是否出现过
    let mut out: Vec<Message> = Vec::new();
    for m in messages.into_iter().rev() {
        let m = match m {
            Message::ToolResult {
                call_id,
                tool,
                args,
                output,
                error,
            } if error.is_none() => {
                let assets = tool_image_assets(&output);
                if assets.is_empty() {
                    Message::ToolResult {
                        call_id,
                        tool,
                        args,
                        output,
                        error,
                    }
                } else if image_allowed == Some(false) {
                    // 引用号留原地（设计稿「保持文本占位 + asset 引用号」）
                    Message::ToolResult {
                        call_id,
                        tool,
                        args,
                        output: with_image_note(&output, false, unsupported_note()),
                        error,
                    }
                } else {
                    if !user_seen {
                        // 逆序遍历：结果间与结果内都先翻，末尾一次 reverse
                        // 双双还原（实抓：单结果 assets 数组被整体翻面，
                        // 模型看到的页序颠倒）
                        tail_assets.extend(assets.iter().rev().cloned());
                    }
                    Message::ToolResult {
                        call_id,
                        tool,
                        args,
                        output: with_image_note(&output, true, attached_note(&assets)),
                        error,
                    }
                }
            }
            other => other,
        };
        if matches!(m, Message::User { .. }) {
            user_seen = true;
        }
        out.push(m);
    }
    out.reverse();
    if !tail_assets.is_empty() {
        // 逆序收集，落位前翻回消息序
        tail_assets.reverse();
        out.push(Message::User {
            content: TOOL_IMAGE_PREFIX.into(),
            parts: tail_assets
                .into_iter()
                .map(|asset| ContentPart::Image { asset })
                .collect(),
        });
    }
    out
}

/// 合成图片 user 消息的正文前缀（content 数组 text 在前 = 稳定前缀）。
const TOOL_IMAGE_PREFIX: &str = "[工具产生的图片]";

/// 工具 output 里的 image 资产（§8.1 约定：`asset` 单数 / `assets` 数组；
/// mime `image/*` 才算——PDF 引用不走图片投影，原地保留）。
fn tool_image_assets(output: &Value) -> Vec<session_store::AssetRef> {
    let parse = |v: &Value| -> Option<session_store::AssetRef> {
        let id = v.get("id")?.as_str()?.to_string();
        let mime = v.get("mime")?.as_str()?.to_string();
        let name = v.get("name")?.as_str()?.to_string();
        mime.starts_with("image/")
            .then_some(session_store::AssetRef { id, mime, name })
    };
    let mut out = Vec::new();
    if let Some(a) = output.get("asset") {
        if let Some(r) = parse(a) {
            out.push(r);
        }
    }
    if let Some(list) = output.get("assets").and_then(Value::as_array) {
        for a in list {
            if let Some(r) = parse(a) {
                out.push(r);
            }
        }
    }
    out
}

/// 已投影的 note（历史轮与尾部轮同文案 = 字节稳定；指过去发生的事实）。
fn attached_note(assets: &[session_store::AssetRef]) -> String {
    let names: Vec<&str> = assets.iter().map(|a| a.name.as_str()).collect();
    format!("[图片已附于本工具结果之后: {}]", names.join(", "))
}

/// 模型不支持图片时的 note（引用号留原地）。
fn unsupported_note() -> String {
    "[图片未随请求发送：当前模型不支持图片输入]".into()
}

/// output 克隆改写：`strip` 时剥 `asset`/`assets` 键（投影成功后引用号是
/// 噪声；不支持时保留），统一加 `image_note`。
fn with_image_note(output: &Value, strip: bool, note: String) -> Value {
    let Some(mut obj) = output.as_object().cloned() else {
        return output.clone();
    };
    if strip {
        obj.remove("asset");
        obj.remove("assets");
    }
    obj.insert("image_note".into(), Value::String(note));
    Value::Object(obj)
}

/// 中立投影 → 线上消息。Assistant 的正文 + 工具调用合成一条消息（协议要求
/// tool 结果挂在发起调用的同一条 assistant 消息上）；ToolResult 的错误作为
/// JSON 内容回传（协议没有独立错误位，模型可自行处理）。
/// User 带 parts（批 D）→ content 数组：正文 text 在前（稳定前缀，M1 缓存
/// 纪律——同图同 asset 同字节 + 图片只追加在尾部 = 追加式变更）+
/// `image_url` data URL / `file`（PDF）按 parts 序。**空 parts = 字符串
/// content（字节与现网逐字节一致）**。
fn message_value(
    m: &Message,
    assets: Option<&crate::assets::AssetStore>,
    budget: Option<usize>,
) -> Result<Value, String> {
    match m {
        Message::User { content, parts } => {
            if parts.is_empty() {
                return Ok(json!({ "role": "user", "content": content }));
            }
            let mut arr: Vec<Value> = Vec::with_capacity(parts.len() + 1);
            if !content.is_empty() {
                arr.push(json!({ "type": "text", "text": content }));
            }
            for p in parts {
                match p {
                    ContentPart::Text { text } => {
                        if !text.is_empty() {
                            arr.push(json!({ "type": "text", "text": text }));
                        }
                    }
                    ContentPart::Image { asset } => {
                        let url = asset_data_url(assets, asset)?;
                        arr.push(json!({ "type": "image_url", "image_url": { "url": url } }));
                    }
                    // 兼容层 file part 形态（OpenAI：file_data data URL +
                    // filename；Anthropic/Google 兼容层各自映射）
                    ContentPart::File { asset } => {
                        let url = asset_data_url(assets, asset)?;
                        arr.push(json!({
                            "type": "file",
                            "file": { "filename": asset.name, "file_data": url },
                        }));
                    }
                }
            }
            Ok(json!({ "role": "user", "content": arr }))
        }
        Message::Assistant {
            content,
            tool_calls,
        } => {
            if tool_calls.is_empty() {
                Ok(json!({ "role": "assistant", "content": content }))
            } else {
                Ok(json!({
                    "role": "assistant",
                    "content": if content.is_empty() { Value::Null } else { json!(content) },
                    "tool_calls": tool_calls.iter().map(|tc| json!({
                        "id": tc.id.0,
                        "type": "function",
                        "function": {
                            "name": tc.tool,
                            "arguments": tc.args.to_string(),
                        },
                    })).collect::<Vec<_>>(),
                }))
            }
        }
        Message::ToolResult {
            call_id,
            output,
            error,
            ..
        } => {
            // 错误支也过预算：错误正文可能带一整段 stderr/堆栈，形状与成功支
            // 同属"一条 tool content"，只截成功支等于留一个绕开闸门的侧门。
            let content = match error {
                Some(e) => clip_tool_output(json!({ "error": e }).to_string(), budget),
                None => clip_tool_output(output.to_string(), budget),
            };
            Ok(json!({ "role": "tool", "tool_call_id": call_id.0, "content": content }))
        }
    }
}

/// asset → data URL（LRU 缓存在 AssetStore 内）。注入缺席 = Fatal（组装
/// 点没接资产存储却收到带附件的请求——测试路径错误配置，如实报错不静默丢）。
fn asset_data_url(
    assets: Option<&crate::assets::AssetStore>,
    asset: &session_store::AssetRef,
) -> Result<String, String> {
    let store =
        assets.ok_or_else(|| format!("资产存储未接入，附件 {} 无法随请求发送", asset.name))?;
    store.data_url(&asset.id, &asset.mime)
}

/// 单个 SSE chunk 对累积器的作用（delta 为切片副本——token 级小串，
/// 拷贝换掉自引用生命周期的麻烦）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    Content(String),
    Reasoning(String),
    None,
}

#[derive(Default, Clone)]
struct ToolAcc {
    id: String,
    name: String,
    args: String,
}

/// 流式累积器：正文 / 推理 / 工具调用分片（按 index 拼接）/ finish_reason /
/// usage。空 id 的调用在收束时按序补号（本地模型常见，§6.3 关联键需要）。
#[derive(Default)]
pub struct StreamAcc {
    content: String,
    tools: BTreeMap<u32, ToolAcc>,
    finish_reason: Option<String>,
    usage: Option<LlmUsage>,
}

impl StreamAcc {
    pub fn new() -> Self {
        Self::default()
    }

    /// 应用一个 chunk，返回需要实时外推的 delta（正文 / 推理）。解析失败或
    /// 与正文无关的块（如纯 usage 尾块）返回 `None`。
    pub fn apply(&mut self, chunk: &Value) -> Delta {
        if let Some(u) = chunk.get("usage") {
            self.usage = parse_usage(u).or(self.usage.take());
        }
        let Some(choice) = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
        else {
            return Delta::None;
        };
        if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(fr.to_string());
        }
        let Some(delta) = choice.get("delta") else {
            return Delta::None;
        };
        // 推理先行（reasoning_content 是 DeepSeek/Qwen/GLM 等的事实标准；
        // 个别网关用 reasoning）——推理不进正文
        for key in ["reasoning_content", "reasoning"] {
            if let Some(r) = delta.get(key).and_then(Value::as_str) {
                if !r.is_empty() {
                    return Delta::Reasoning(r.to_string());
                }
            }
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tc in tool_calls {
                self.apply_tool_call(tc);
            }
        }
        match delta.get("content").and_then(Value::as_str) {
            Some(c) if !c.is_empty() => {
                self.content.push_str(c);
                Delta::Content(c.to_string())
            }
            _ => Delta::None,
        }
    }

    /// `delta.tool_calls[]` 单项：按 index 拼接 id / function.name /
    /// arguments（参数分片传输）。
    fn apply_tool_call(&mut self, tc: &Value) {
        let index = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
        let acc = self.tools.entry(index).or_default();
        if let Some(id) = tc.get("id").and_then(Value::as_str) {
            if !id.is_empty() {
                acc.id = id.to_string();
            }
        }
        if let Some(f) = tc.get("function") {
            if let Some(n) = f.get("name").and_then(Value::as_str) {
                if !n.is_empty() {
                    acc.name = n.to_string();
                }
            }
            if let Some(a) = f.get("arguments").and_then(Value::as_str) {
                acc.args.push_str(a);
            }
        }
    }

    /// 本轮是否已向外推过任何正文/推理 delta（重试重置判定用）。
    pub fn finish_reason(&self) -> Option<&str> {
        self.finish_reason.as_deref()
    }

    /// 收束：正文 + 工具调用（参数字符串解析回 JSON Value；空参 = `{}`；
    /// 解析失败的原始串原样传递——工具执行会以清晰错误回传，不静默丢弃）+
    /// 用量。空 call_id 按序补号。
    pub fn finish(self, next_call: &AtomicU64) -> LlmResponse {
        let tool_calls = self
            .tools
            .into_values()
            .map(|acc| {
                let id = if acc.id.is_empty() {
                    format!("call-{}", next_call.fetch_add(1, Ordering::Relaxed))
                } else {
                    acc.id
                };
                let args = if acc.args.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&acc.args)
                        .unwrap_or_else(|_| Value::String(acc.args.clone()))
                };
                ToolCallReq {
                    id: CallId(id),
                    tool: acc.name,
                    args,
                    session_id: None,
                }
            })
            .collect();
        LlmResponse {
            content: self.content,
            tool_calls,
            usage: self.usage,
        }
    }
}

/// 用量映射（LLM.md 字段表）：input/output/total 基础计数；缓存读 =
/// OpenAI `prompt_tokens_details.cached_tokens` 或 DeepSeek
/// `prompt_cache_hit_tokens`；缓存写 = `cache_creation_tokens`（Anthropic
/// 形态，有则取）；推理 = `completion_tokens_details.reasoning_tokens`。
/// 全 0 = None（供应商没报有效用量，宁缺勿假）。
pub fn parse_usage(u: &Value) -> Option<LlmUsage> {
    let num = |v: &Value| -> u64 { v.as_i64().unwrap_or(0).max(0) as u64 };
    let input = num(&u["prompt_tokens"]);
    let output = num(&u["completion_tokens"]);
    if input == 0 && output == 0 {
        return None;
    }
    let prompt_d = &u["prompt_tokens_details"];
    let completion_d = &u["completion_tokens_details"];
    Some(LlmUsage {
        input,
        output,
        cache_read: num(&prompt_d["cached_tokens"]).max(num(&u["prompt_cache_hit_tokens"])),
        cache_write: num(&prompt_d["cache_creation_tokens"]),
        reasoning: num(&completion_d["reasoning_tokens"]),
        total: num(&u["total_tokens"]),
    })
}
