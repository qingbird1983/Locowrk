//! ctx-compact（plugins.md P1，第二批第 8 项）：`agent/request` waterfall
//! 监听器——估算 token 超阈值就把早期消息压缩成单条摘要。
//!
//! 与 mw-audit 同理落在宿主侧：C ABI 插件够不到 LlmService / EventBus。
//! 摘要调用**直连** LlmService（不走 `agent/request` 决策环——否则自我递归）。
//! fail-open：估算/摘要任何一步失败都放行原 payload，宁可不压也不让请求失败。
//!
//! 切点纪律：压缩边界必须落在 User 消息之前——把 Assistant(tool_calls) 与
//! 它的 ToolResult 拆到摘要两侧会破坏真实模型协议（tool_call_id 配对）。

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{Context, WaterfallDecision, WaterfallListener};
use serde_json::Value;
use session_store::Message;

use crate::kernel::{LlmRequest, LlmService};
use crate::waterfall::{RequestPayload, points};

/// 压缩参数。`max_tokens` 是估算值：序列化字节 / 4（英文近似 token 数；
/// CJK 偏低估，宁早压不爆仓）。`keep_recent` 是保留的近期消息条数下限。
#[derive(Debug, Clone)]
pub struct CompactConfig {
    pub max_tokens: u64,
    pub keep_recent: usize,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            max_tokens: 32_000,
            keep_recent: 8,
        }
    }
}

/// 挂在 `agent/request` 上的压缩监听器。构造时持有 Context（运行时才解析
/// LlmService——装配时 LLM 适配器可能尚未注册）。
pub struct CtxCompact {
    ctx: Arc<Context>,
    cfg: CompactConfig,
}

impl CtxCompact {
    pub fn new(ctx: Arc<Context>, cfg: CompactConfig) -> Self {
        Self { ctx, cfg }
    }
}

#[async_trait]
impl WaterfallListener for CtxCompact {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
        if estimate(&req.messages) <= self.cfg.max_tokens {
            return WaterfallDecision::Continue;
        }
        let cut = safe_cut(&req.messages, self.cfg.keep_recent);
        if cut == 0 {
            return WaterfallDecision::Continue; // 没有可压边界（不该硬压）
        }
        let Ok(llm) = self.ctx.use_dyn::<dyn LlmService>() else {
            return WaterfallDecision::Continue;
        };
        let transcript = match serde_json::to_string(&req.messages[..cut]) {
            Ok(t) => t,
            Err(_) => return WaterfallDecision::Continue,
        };
        let summary_call = LlmRequest {
            messages: vec![Message::User {
                content: format!(
                    "Summarize the earlier conversation below into concise notes for \
                     continuing the work. Keep key facts, decisions, file paths and \
                     unfinished tasks. Reply with the summary only.\n\n<transcript>\n\
                     {transcript}"
                ),
            }],
            tools: vec![],
            model: req.model.clone(),
            params: Value::Object(serde_json::Map::new()),
        };
        let summary = match llm.complete(summary_call).await {
            Ok(resp) if !resp.content.trim().is_empty() => resp.content,
            _ => return WaterfallDecision::Continue, // fail-open
        };
        req.messages = [Message::User {
            content: format!("[earlier conversation summarized]\n{summary}"),
        }]
        .into_iter()
        .chain(req.messages[cut..].iter().cloned())
        .collect();
        // Replace 是逐跳改写：终态决策仍是 Continue，改写结果在 payload 里
        // （cordis-core::waterfall 语义）。to_value 失败按 fail-open 放行。
        match serde_json::to_value(&req) {
            Ok(v) => WaterfallDecision::Replace(v),
            Err(_) => WaterfallDecision::Continue,
        }
    }
}

/// 粗估 token：全消息序列化字节 / 4（转义带来的高估当安全余量）。
fn estimate(messages: &[Message]) -> u64 {
    messages
        .iter()
        .map(|m| serde_json::to_string(m).map_or(0, |s| s.len() as u64))
        .sum::<u64>()
        / 4
}

/// 找压缩切点：目标 `len - keep_recent`，向前后调整到最近的 User 起点
/// （切在 User 之前不会拆散 tool_calls ↔ ToolResult 配对）。
/// 返回 0 表示找不到边界（不压缩）。
fn safe_cut(messages: &[Message], keep_recent: usize) -> usize {
    let len = messages.len();
    let k = len.saturating_sub(keep_recent);
    if k == 0 {
        return 0;
    }
    let is_user = |m: &Message| matches!(m, Message::User { .. });
    if is_user(&messages[k]) {
        return k;
    }
    if let Some(j) = (k + 1..len).find(|&j| is_user(&messages[j])) {
        return j;
    }
    (1..k).rev().find(|&j| is_user(&messages[j])).unwrap_or(0)
}

#[cfg(test)]
mod tests;
