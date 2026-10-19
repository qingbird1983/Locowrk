//! 请求装配与它带的两份上限：`WireCaps` = 模型能力上限（用户钉死 max_output /
//! 生效 ctx 窗口 / Q8 工具结果预算 + 刀② 的落盘句柄，预算的 KiB→字节算式只有
//! 一份，在本层不重乘），`build_payload` = 中立投影直转成线上 body
//! （`stream_options` 只在流式带、max_tokens 只在钉死值在场时发并按
//! `min(钉死, ctx − 估用 − 1000)` 预检钳制、推理档位按方言写字段），
//! `project_tool_output` = 超预算工具输出的**一处判定、两种后果**
//! （能落盘就留 `<persisted-output>` 路径 + 预览，落不了就退回改前那句截断提示）。

use std::sync::Arc;

use serde_json::{json, Value};

use kernel_agent::{estimate_tokens, LlmRequest};
use plugin_api::ToolSpec;

use super::messages::message_value;
use crate::llm::dialect::Dialect;
use crate::spill::SpillStore;

/// 模型能力上限（批 B，AppLlm 按 settings 钉死面解析后传入；profile 级
/// 拿不到 = 全 None，行为与 v2.75 一致——不发明派生限额）。
///
/// 刀② 起带一个 `Arc`（落盘句柄），所以**不再是 `Copy`**：本结构原来靠 Copy
/// 省掉引用写法，现在多一个借用点（`&caps` 在 `attempt_stream` 里本来就有），
/// 换来的是"发送侧的上下文整形通路都在同一份 caps 上"，不必给
/// `complete_with` / `complete_streaming_with` / `build_payload` 再各加一枚参数。
#[derive(Debug, Clone, Default)]
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
    /// S2 刀②：超预算工具输出的落盘处。None = 不落盘 ⇒ 行为退回改前的纯截断
    /// （headless / 测试装配 / 设置里把预算关掉都走这一档）。
    pub spill: Option<Arc<SpillStore>>,
}

/// Q8 + S2 刀②：工具结果超过预算时的**唯一一处尺寸判定**。
///
/// 三点是刻意的：①只动发给模型的这一份，会话日志仍是全量真相源（回捞 / 导出 /
/// 崩溃重放读它）；②切点回退到 UTF-8 字符边界（算式在 `spill::char_boundary_cut`，
/// 中文/emoji 按字节硬切会当场 panic）；③`None`（预算 0 = 不限制）时原样返回 =
/// 与改动前逐字节一致，这条等价关系由 `llm/tests/tool_budget.rs` 钉着。
///
/// 为什么"落盘"与"截断"必须是同一个函数的两个后果，而不是两层：两层 = 两个尺寸
/// 判定，界面上就是一枚设置管两层（"预算 24K 到底是截断线还是落盘线"没人说得清），
/// 而方案稿要的 spill 与现有 Q8 预算本来就是同一件事的两种后果。
///
/// 错误支也过这里：错误正文可能带一整段 stderr/堆栈，只放成功支等于留一个
/// 绕开闸门的侧门（`messages.rs` 头注同一条纪律）。
pub(super) fn project_tool_output(
    content: String,
    budget: Option<usize>,
    spill: Option<(&SpillStore, &str)>,
) -> String {
    let Some(limit) = budget else {
        return content;
    };
    if content.len() <= limit {
        return content;
    }
    let cut = crate::spill::char_boundary_cut(&content, limit);
    if let Some((store, session_id)) = spill {
        match store.put(session_id, &content) {
            Ok(path) => return crate::spill::persisted(&path, &content[..cut], cut, content.len()),
            // 落不下来就如实退回截断，并把失败原因喊出来——"静默当成落了"是
            // 比丢内容更坏的一种（模型会照着不存在的路径去读）。
            Err(e) => tracing::warn!("工具结果落盘失败，按截断发送：{e}"),
        }
    }
    crate::spill::clipped(&content, cut)
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
    // 刀②：落盘需要两件事**同时**在场——句柄（组装点注入了）+ 本请求的会话身份。
    // 缺一件就退回纯截断而不是"猜一个目录"：spill 文件按会话分子目录正是为了让
    // 它能被追认与清理，没有会话 id 的落盘等于往数据根扔无主文件。
    let spill = match (caps.spill.as_deref(), req.session_id.as_deref()) {
        (Some(store), Some(sid)) => Some((store, sid)),
        _ => None,
    };
    for m in &req.messages {
        messages.push(message_value(
            m,
            assets,
            caps.tool_result_budget_bytes,
            spill,
        )?);
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
