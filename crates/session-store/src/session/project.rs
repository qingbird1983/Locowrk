//! 事件日志 → 消息序列的投影与收束（自 `session.rs` 批 F-17 拆出，函数体逐字未改）：
//! `project_messages`（含 ⑳ 上下文摘要段、G2 悬空调用的合成 ToolResult、
//! ㉕·fix 的 UserMessage 补齐）/ `compact_messages`（整理后的原地收束）/
//! `backfill`（ToolResult 缺字段时的兜底）。

use super::{Message, ToolCallMsg};
use crate::event::{SessionEvent, UncertainDecision};
use crate::uncertain::closure;
use plugin_api::CallId;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 压缩折叠（Context and Memory.md §3.7）：留尾部 `kept` 条，其余替换为
/// 一条摘要 User 消息（滑动窗口摘要为空串 → 不留摘要，纯丢早期）。
/// `kept >= len`（无头部可压）时空操作。apply（快照折叠）与
/// derive_messages 共用。
/// 事件切片 → 消息投影（`derive_messages` / `derive_messages_from` 共用
/// 单点实现，两遍：ToolCall 索引 → 拼装；M4 自 derive_messages 抽出）。
/// G2 悬空闭合：assistant 声明了 call_id 而切片内没有 ToolResult 事件
/// （进程在执行中退出的悬空调用，可靠性稿 §5）→ 紧贴 assistant 之后
/// 合成一条诚实闭合的 ToolResult 消息（未裁决 =「效果未知」；有裁决 =
/// 按裁决闭合）——不给合成，下一轮请求带悬空 tool_calls 会被供应商
/// 整单拒收。快照折叠（`apply`）不合成：快照只服务存档面，请求装配
/// 唯一真源是本函数。
pub(super) fn project_messages(events: &[SessionEvent]) -> Vec<Message> {
    let mut calls: HashMap<&CallId, (&str, &Value)> = HashMap::new();
    let mut closed: HashSet<&CallId> = HashSet::new();
    let mut decisions: HashMap<&CallId, (UncertainDecision, &str)> = HashMap::new();
    for e in events {
        match e {
            SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                ..
            } => {
                calls.insert(call_id, (tool.as_str(), args));
            }
            SessionEvent::ToolResult { call_id, .. } => {
                closed.insert(call_id);
            }
            SessionEvent::ToolExecutionUncertain {
                call_id,
                decision,
                note,
                ..
            } => {
                decisions.insert(call_id, (*decision, note.as_str()));
            }
            _ => {}
        }
    }

    let mut messages: Vec<Message> = Vec::new();
    for e in events {
        match e {
            SessionEvent::UserMessage { content, parts } => messages.push(Message::User {
                content: content.clone(),
                parts: parts.clone(),
            }),
            SessionEvent::AssistantMessage {
                content,
                tool_calls,
            } => {
                messages.push(Message::Assistant {
                    content: content.clone(),
                    tool_calls: tool_calls
                        .iter()
                        .map(|id| {
                            let (tool, args) = backfill(calls.get(id));
                            ToolCallMsg {
                                id: id.clone(),
                                tool,
                                args,
                            }
                        })
                        .collect(),
                });
                // 悬空闭合（G2）：声明位没有 ToolResult 事件 = 按裁决或
                // 「效果未知」合成。位置紧贴 assistant——tool_calls 与
                // tool 消息之间的先后是供应商硬约束。
                for id in tool_calls {
                    if !closed.contains(id) {
                        let (tool, args) = backfill(calls.get(id));
                        let (output, error) = closure(decisions.get(id).map(|(d, n)| (*d, *n)));
                        messages.push(Message::ToolResult {
                            call_id: id.clone(),
                            tool,
                            args,
                            output,
                            error,
                        });
                    }
                }
            }
            SessionEvent::ToolResult {
                call_id,
                output,
                error,
            } => {
                let (tool, args) = backfill(calls.get(call_id));
                messages.push(Message::ToolResult {
                    call_id: call_id.clone(),
                    tool,
                    args,
                    output: output.clone(),
                    error: error.clone(),
                });
            }
            SessionEvent::ContextCompacted {
                kept_messages,
                summary,
                ..
            } => compact_messages(&mut messages, *kept_messages, summary),
            // 整理段落总结（Organize Summary Design v1 §3.2）：映成带标记的
            // User 消息（ContextCompacted 摘要同款纯标记先例——session-store
            // 无 system-reminder 包裹惯例，免转义面）。位置 = 日志真实位
            // （断层缝合处），从落位轮起随历史每轮在场。
            SessionEvent::ContextSummary { text } => messages.push(Message::user(format!(
                "[context summary of earlier organized conversation]\n{text}"
            ))),
            _ => {}
        }
    }
    messages
}

pub(super) fn compact_messages(messages: &mut Vec<Message>, kept: usize, summary: &str) {
    let keep = kept.min(messages.len());
    let head_len = messages.len() - keep;
    let tail = messages.split_off(head_len);
    let head = std::mem::take(messages);
    if !summary.is_empty() && !head.is_empty() {
        messages.push(Message::user(format!(
            "[earlier conversation summarized]\n{summary}"
        )));
    }
    messages.extend(tail);
}

fn backfill(found: Option<&(&str, &Value)>) -> (String, Value) {
    found
        .map(|(t, a)| ((*t).to_string(), (*a).clone()))
        .unwrap_or_else(|| (String::new(), Value::Null))
}
