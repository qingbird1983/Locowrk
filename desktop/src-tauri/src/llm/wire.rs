//! 线上协议（OpenAI Chat Completions，LLM.md v3.0）：请求装配 / SSE chunk
//! 解析 / 用量映射，纯函数 + 流式累积器。**唯一线路**——非流式 `complete`
//! 也走这条管道聚合（无第二形状）。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use session_store::Message;

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
}

/// 预检钳制下限：估用已吃掉空间时不钳到负数，留给上下文溢出语义接管。
const MIN_MAX_TOKENS: u64 = 1024;
/// 预检钳制的安全余量（供应商稿 §6）。
const CLAMP_HEADROOM: i64 = 1000;

/// 请求装配（LLM.md「请求装配」节）：中立投影直转；`stream_options` 只在
/// 流式携带；**max_tokens 只在用户钉死值在场时发送 + 预检钳制**
/// `min(钉死, ctx − 估用 − 1000)`（供应商稿 §6 窄修订，D5 裁定）；推理
/// 档位按方言写字段（`dialect.patch`，映射不了不发）。
pub fn build_payload(
    model: &str,
    req: &LlmRequest,
    stream: bool,
    dialect: Dialect,
    caps: &WireCaps,
) -> Value {
    let mut messages: Vec<Value> = Vec::with_capacity(req.messages.len() + 1);
    if let Some(sys) = &req.system {
        messages.push(json!({ "role": "system", "content": sys }));
    }
    for m in &req.messages {
        messages.push(message_value(m));
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
    payload
}

/// 发送前能力门控（供应商稿 §7 第二道防线；上传侧第一道在批 D 前端）：
/// 请求 content parts 的模态 ⊆ 模型 input_modalities。批 D 落地前
/// `Message::User.content` 是纯文本 String（构造上恒过）——检查点先立起
/// 来，D 批 ContentPart 落地后在此填模态比对。
pub fn check_modalities(_req: &LlmRequest) -> Result<(), String> {
    Ok(())
}

/// 中立投影 → 线上消息。Assistant 的正文 + 工具调用合成一条消息（协议要求
/// tool 结果挂在发起调用的同一条 assistant 消息上）；ToolResult 的错误作为
/// JSON 内容回传（协议没有独立错误位，模型可自行处理）。
fn message_value(m: &Message) -> Value {
    match m {
        Message::User { content } => json!({ "role": "user", "content": content }),
        Message::Assistant {
            content,
            tool_calls,
        } => {
            if tool_calls.is_empty() {
                json!({ "role": "assistant", "content": content })
            } else {
                json!({
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
                })
            }
        }
        Message::ToolResult {
            call_id,
            output,
            error,
            ..
        } => {
            let content = match error {
                Some(e) => json!({ "error": e }).to_string(),
                None => output.to_string(),
            };
            json!({ "role": "tool", "tool_call_id": call_id.0, "content": content })
        }
    }
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
