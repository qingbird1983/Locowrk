//! 记录员材料（§4.4「交给记录员什么」）：事件切片 → 带行号标记的转录
//! 文本。行号 = **材料项下标**（0 起，逐项连续）——记录员按它切分，
//! 系统再翻回事件位存 `event_range`（流内过程事件如 LlmUsage 不进材料，
//! 直接用事件位做切分空间会被它们打碎连续性）。
//!
//! token 估算与现网同一把尺：serde 序列化字节 / 4（kernel_agent::
//! estimate_tokens 同式；本 crate 不依赖 kernel-agent，就地同式实现）。

use serde::{Deserialize, Serialize};
use session_store::SessionEvent;

use crate::bundle::EventId;

/// 工具参数 / 结果在材料里的截断宽度（记录员只需要轮廓，原文在流里）。
const TOOL_CLIP: usize = 400;
/// 助手正文的截断宽度（用户消息永不截——逐字保留纪律 §5.6）。
const ASSISTANT_CLIP: usize = 6000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    User,
    Assistant,
    ToolCall,
    ToolResult,
}

/// 一条材料项：`event` 是它在会话日志里的绝对位（L2 引用的锚点）。
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialItem {
    /// 材料内下标（0 起）——记录员的切分空间。
    pub index: usize,
    /// 会话日志事件位。
    pub event: EventId,
    pub kind: MaterialKind,
    pub text: String,
    pub tokens: u64,
}

fn clip(s: &str, n: usize) -> String {
    let t = s.trim();
    let c: String = t.chars().take(n).collect();
    if t.chars().count() > n {
        format!("{c}…")
    } else {
        c.to_string()
    }
}

fn est(s: &str) -> u64 {
    (s.len() as u64) / 4
}

/// 事件切片 → 材料项。`base` 是切片首元素在会话日志里的绝对位（调用方
/// 传尾巴切片时带出偏移，产出的 `event` 恒为日志绝对位）。过程事件
/// （Thinking / LlmUsage / ContextUsage / ApprovalDecided / TurnStart/End /
/// Effect / Plugin）不进材料——它们不是对话内容，决定与结论都在
/// user/assistant/tool 四类里；空正文且无工具调用的 assistant 步同此。
pub fn build_items(events: &[SessionEvent], base: EventId) -> Vec<MaterialItem> {
    let mut out = Vec::new();
    for (i, e) in events.iter().enumerate() {
        let pos = base + i;
        let (kind, text) = match e {
            SessionEvent::UserMessage { content } => (MaterialKind::User, content.clone()),
            SessionEvent::AssistantMessage {
                content,
                tool_calls,
            } => {
                let mut text = content.clone();
                if text.trim().is_empty() && !tool_calls.is_empty() {
                    text = "（本步无正文，只发起工具调用）".into();
                }
                (MaterialKind::Assistant, text)
            }
            SessionEvent::ToolCall { tool, args, .. } => {
                let args = serde_json::to_string(args).unwrap_or_default();
                (
                    MaterialKind::ToolCall,
                    format!("{tool}: {}", clip(&args, TOOL_CLIP)),
                )
            }
            SessionEvent::ToolResult { output, error, .. } => {
                let body = match error {
                    Some(e) => format!("错误：{e}"),
                    None => clip(&output.to_string(), TOOL_CLIP),
                };
                (MaterialKind::ToolResult, body)
            }
            _ => continue,
        };
        if matches!(kind, MaterialKind::User | MaterialKind::Assistant) && text.trim().is_empty() {
            continue;
        }
        out.push(MaterialItem {
            index: out.len(),
            event: pos,
            tokens: est(&text),
            kind,
            text,
        });
    }
    out
}

/// 材料项 → 转录文本：每行 `[下标] 类型: 内容`（提示词约定 event_range
/// 用行首下标，含首尾）。
pub fn transcript(items: &[MaterialItem]) -> String {
    let mut s = String::new();
    for it in items {
        let kind = match it.kind {
            MaterialKind::User => "用户",
            MaterialKind::Assistant => "助手",
            MaterialKind::ToolCall => "工具调用",
            MaterialKind::ToolResult => "工具结果",
        };
        let text: String = if it.kind == MaterialKind::User {
            it.text.clone() // 用户消息逐字
        } else {
            clip(&it.text, ASSISTANT_CLIP)
        };
        s.push_str(&format!("[{}] {kind}: {text}\n", it.index));
    }
    s
}

/// 尾巴 token 估算（增量档触发与观测用）。
pub fn tail_tokens(items: &[MaterialItem]) -> u64 {
    items.iter().map(|i| i.tokens).sum()
}

/// 会话切片里的用户消息数（反弹熔断的"轮"锚点，§5.5）。
pub fn user_count(events: &[SessionEvent]) -> u64 {
    events
        .iter()
        .filter(|e| matches!(e, SessionEvent::UserMessage { .. }))
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_api::CallId;

    fn events() -> Vec<SessionEvent> {
        use serde_json::json;
        use std::sync::OnceLock;
        fn ver() -> &'static semver::Version {
            static V: OnceLock<semver::Version> = OnceLock::new();
            V.get_or_init(|| semver::Version::new(0, 1, 0))
        }
        vec![
            SessionEvent::UserMessage {
                content: "帮我查插件加载".into(),
            },
            SessionEvent::LlmUsage {
                input: 1,
                output: 1,
                cache_read: 0,
                cache_write: 0,
                reasoning: 0,
                total: 2,
                duration_ms: 1,
            },
            SessionEvent::AssistantMessage {
                content: String::new(),
                tool_calls: vec![],
            },
            SessionEvent::ToolCall {
                call_id: CallId("c1".into()),
                tool: "fs_read".into(),
                args: json!({"path": "a.rs"}),
                plugin_id: None,
                plugin_version: ver().clone(),
                side_effect: plugin_api::SideEffect::Pure,
            },
            SessionEvent::ToolResult {
                call_id: CallId("c1".into()),
                output: json!({"ok": true}),
                error: None,
            },
            SessionEvent::AssistantMessage {
                content: "加载慢的原因是 X".into(),
                tool_calls: vec![],
            },
        ]
    }

    #[test]
    fn build_items_skips_process_events_and_reindexes() {
        let items = build_items(&events(), 0);
        let kinds: Vec<&MaterialKind> = items.iter().map(|i| &i.kind).collect();
        // 事件 2 = 空正文且无工具调用的 assistant 步 → 无内容可记，跳过
        assert_eq!(
            kinds,
            vec![
                &MaterialKind::User,
                &MaterialKind::ToolCall,
                &MaterialKind::ToolResult,
                &MaterialKind::Assistant
            ]
        );
        // 事件位保留原流位置（LlmUsage 位 1 / 空 assistant 位 2 被跳过）
        assert_eq!(items[0].event, 0);
        assert_eq!(items[1].event, 3);
        assert_eq!(items[2].event, 4);
        // 材料下标连续
        assert_eq!(items[3].index, 3);
    }

    #[test]
    fn transcript_labels_lines_with_item_index() {
        let items = build_items(&events(), 0);
        let t = transcript(&items);
        assert!(t.contains("[0] 用户: 帮我查插件加载"));
        assert!(t.contains("[1] 工具调用: fs_read:"));
    }

    #[test]
    fn tail_tokens_sums_estimates() {
        let items = build_items(&events(), 0);
        assert_eq!(
            tail_tokens(&items),
            items.iter().map(|i| i.tokens).sum::<u64>()
        );
    }

    #[test]
    fn user_count_only_user_messages() {
        assert_eq!(user_count(&events()), 1);
    }
}
