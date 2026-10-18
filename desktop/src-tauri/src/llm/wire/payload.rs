//! 请求装配与它带的两份上限：`WireCaps` = 模型能力上限（用户钉死 max_output /
//! 生效 ctx 窗口 / Q8 工具结果预算，预算的 KiB→字节算式只有一份，在本层不重乘），
//! `build_payload` = 中立投影直转成线上 body（`stream_options` 只在流式带、
//! max_tokens 只在钉死值在场时发并按 `min(钉死, ctx − 估用 − 1000)` 预检钳制、
//! 推理档位按方言写字段），`clip_tool_output` = 超预算工具输出的字节截断 + 可续读提示。

use serde_json::{json, Value};

use kernel_agent::{estimate_tokens, LlmRequest};
use plugin_api::ToolSpec;

use super::messages::message_value;
use crate::llm::dialect::Dialect;

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
pub(super) fn clip_tool_output(content: String, budget: Option<usize>) -> String {
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
