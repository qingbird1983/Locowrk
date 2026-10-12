//! 会话日志 → UI 事件的投影（自 `runner.rs` 批 F-17 拆出，函数体逐字未改）。
//! 单点实现："UI 事件是会话日志的投影，不是第二真相源"——增量投影（run_turn 内
//! 的 project_new_events）与 ⑲ 全量恢复投影（session_history 命令）吃同一个
//! `project_events`。G2 悬空扫描与 ㉕·对话区① 的 call_id 找回也在这层。

use crate::events::{side_effect_str, AgentEvent};
use serde_json::Value;
use session_store::SessionEvent;
use std::collections::{HashMap, HashSet};

/// 事件切片 → UI 事件序列（纯函数）。增量投影（run_turn）与 ⑲ 全量恢复
/// 投影（session_history 命令）共用同一映射——"UI 事件是会话日志的投影"
/// 的单点实现。
pub fn project_events(events: &[SessionEvent], session_id: &str) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    // ApprovalDecided 只有 call_id（㉕·对话区①）：tool 从同切片的 ToolCall
    // 按 call_id 找回——ToolCall 先行落日志（纪律 4），同一步内必在决策之前。
    let mut call_tools: HashMap<String, String> = HashMap::new();
    // TurnStart.ts 配给紧随的 UserMessage（v2.101）：重放计时起点。消息
    // 编辑重发截断留下的孤儿 TurnStart 会被下一个 TurnStart 覆盖（自愈，
    // 不会把旧轮的起点错配给新轮）。
    let mut turn_ts: Option<i64> = None;
    // G2 悬空扫描：声明了 call_id 而无 ToolResult 事件的调用（进程执行中
    // 退出）——工具行后补 ToolUncertain 标记（卡），裁决事件投影收束。
    // has_call_event：只落了声明位、连 ToolCall 事件都没有的调用（死在
    // 两次落盘之间）没有宿主工具行，卡直接挂在 assistant 段之后。
    let uncertain = session_store::scan_uncertain(events);
    // 批 F-24b / M-18e①：这张表**只**被下面那一个查询点消费，而它的守卫
    // `uncertain.dangling.contains(..)` 短路在前 ⇒ 正常会话里 `dangling` 恒空，改前却
    // 每次投影都把整条日志的 `call_id` 逐个 clone 建一遍表 = 纯白付。改懒建：
    // 悬空集非空才扫（有悬空 = 崩溃恢复场景，本来就要付）。
    let has_call_event: Option<HashSet<String>> = (!uncertain.dangling.is_empty()).then(|| {
        events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::ToolCall { call_id, .. } => Some(call_id.0.clone()),
                _ => None,
            })
            .collect()
    });
    for (pos, e) in events.iter().enumerate() {
        match e {
            SessionEvent::TurnStart { ts, .. } => turn_ts = Some(*ts),
            // ㉕·fix：用户消息进投影（此前落 `_` 被丢，重启后历史会话
            // 只剩模型回复）。实时轮不受影响——run_turn 的投影起点在
            // UserMessage 追加之后，该段只落盘不上事件通道（前端本地回显）。
            // `pos`（v2.57 §3.34）：消息编辑重发的定位键——session_history
            // 传全量数组，pos 即绝对事件位。`ts`（v2.101）：轮起点时刻。
            SessionEvent::UserMessage { content, parts } => {
                out.push(AgentEvent::UserMessage {
                    session_id: session_id.into(),
                    content: content.clone(),
                    pos,
                    ts: turn_ts.take().filter(|t| *t != 0),
                    parts: parts.clone(),
                });
            }
            // 推理（v2.57 §3.9）：投影一行 ThinkingRow；实时轮已被流式
            // 消费的 step 由 project_new_events 抑制（重放照常产出）。
            SessionEvent::Thinking { text } => {
                out.push(AgentEvent::Thinking {
                    session_id: session_id.into(),
                    text: text.clone(),
                });
            }
            // 单步用量（v2.58 §3.12）：实时与重放同形——前端按轮聚合；
            // 不需要抑制（无 delta 直推对应物）。
            SessionEvent::LlmUsage {
                input,
                output,
                cache_read,
                cache_write,
                reasoning,
                total,
                duration_ms,
            } => {
                out.push(AgentEvent::LlmUsage {
                    session_id: session_id.into(),
                    input: *input,
                    output: *output,
                    cache_read: *cache_read,
                    cache_write: *cache_write,
                    reasoning: *reasoning,
                    total: *total,
                    duration_ms: *duration_ms,
                });
            }
            SessionEvent::ContextUsage {
                used,
                threshold,
                window_tokens,
                tools_est_tokens,
            } => {
                // v2.62：用量快照重放同形——切会话回来用量环恢复
                out.push(AgentEvent::ContextUsage {
                    session_id: session_id.into(),
                    used: *used,
                    threshold: *threshold,
                    window_tokens: *window_tokens,
                    tools_est_tokens: *tools_est_tokens,
                });
            }
            SessionEvent::AssistantMessage {
                content,
                tool_calls,
            } => {
                // 纯工具调用的 assistant 步正文为空，不发空 chunk
                if !content.is_empty() {
                    out.push(AgentEvent::Chunk {
                        session_id: session_id.into(),
                        delta: content.clone(),
                    });
                }
                // G2：只有声明位、连 ToolCall 事件都没落上的悬空调用
                // （死在两次落盘之间）——没有宿主工具行，卡带空细节直接
                // 出现在 assistant 段之后（细节缺失如实渲染）。已裁决的
                // 也出（重放复现「建卡→裁决收束」序列，同上）。
                for id in tool_calls {
                    if uncertain.dangling.contains(id.0.as_str())
                        && !has_call_event
                            .as_ref()
                            .is_some_and(|s| s.contains(id.0.as_str()))
                    {
                        out.push(AgentEvent::ToolUncertain {
                            session_id: session_id.into(),
                            call_id: id.0.clone(),
                            tool: String::new(),
                            args: Value::Null,
                        });
                    }
                }
            }
            SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                side_effect,
                plugin_version,
                ..
            } => {
                call_tools.insert(call_id.0.clone(), tool.clone());
                out.push(AgentEvent::ToolCall {
                    session_id: session_id.into(),
                    call_id: call_id.0.clone(),
                    tool: tool.clone(),
                    args: args.clone(),
                    side_effect: side_effect_str(side_effect).into(),
                    plugin_version: plugin_version.to_string(),
                });
                // G2：悬空 → 紧随工具行补 uncertain 标记（卡）。已裁决的
                // 也出：重放要复现「建卡→裁决收束」的实时序列（卡落在
                // 裁决事件处收为记录行，行由合成 tool_result 闭合）。
                // tool/args 细节缺失（声明位无 ToolCall 事件）原样透传。
                if uncertain.dangling.contains(call_id.0.as_str()) {
                    out.push(AgentEvent::ToolUncertain {
                        session_id: session_id.into(),
                        call_id: call_id.0.clone(),
                        tool: tool.clone(),
                        args: args.clone(),
                    });
                }
            }
            SessionEvent::ToolResult {
                call_id,
                output,
                error,
            } => out.push(AgentEvent::ToolResult {
                session_id: session_id.into(),
                call_id: call_id.0.clone(),
                ok: error.is_none(),
                output: output.clone(),
                error: error.clone(),
            }),
            SessionEvent::ContextCompacted {
                from_tokens,
                to_tokens,
                strategy,
                summary,
                ..
            } => {
                // 摘要全文在会话日志；UI 只拿 200 字符预览
                out.push(AgentEvent::ContextCompacted {
                    session_id: session_id.into(),
                    from_tokens: *from_tokens,
                    to_tokens: *to_tokens,
                    strategy: strategy.clone(),
                    summary_preview: summary.chars().take(200).collect(),
                });
            }
            // 整理段落总结（Organize Summary Design v1 §3.3）：重放与实时
            // 同形——历史会话恢复时总结块照常出现在断层缝合处。
            SessionEvent::ContextSummary { text } => {
                out.push(AgentEvent::ContextSummary {
                    session_id: session_id.into(),
                    text: text.clone(),
                });
            }
            SessionEvent::ApprovalDecided {
                call_id,
                allowed,
                reason,
            } => {
                // ㉕·对话区① §3.17：决策记录条的唯一来源——实时增量与历史
                // 重放同源，"谁批过什么"重启后也在转录里
                out.push(AgentEvent::ApprovalResolved {
                    session_id: session_id.into(),
                    call_id: call_id.0.clone(),
                    tool: call_tools
                        .get(&call_id.0)
                        .cloned()
                        .unwrap_or_else(|| call_id.0.clone()),
                    allowed: *allowed,
                    reason: reason.clone(),
                });
            }
            SessionEvent::ToolExecutionUncertain {
                call_id,
                decision,
                note,
                ..
            } => {
                // G2：裁决的投影（实时经命令回推 / 重放经此处，同形）。
                // tool 从同切片 ToolCall 找回（声明位缺失回落 call_id，
                // ApprovalResolved 同款）；随后的合成 ToolResult 按裁决
                // 闭合工具行（与裁决命令的实时回推同一来源 closure）。
                out.push(AgentEvent::UncertainResolved {
                    session_id: session_id.into(),
                    call_id: call_id.0.clone(),
                    tool: call_tools
                        .get(call_id.0.as_str())
                        .cloned()
                        .unwrap_or_else(|| call_id.0.clone()),
                    decision: decision.as_str().into(),
                    note: note.clone(),
                });
                let (output, error) = session_store::closure(Some((*decision, note.as_str())));
                out.push(AgentEvent::ToolResult {
                    session_id: session_id.into(),
                    call_id: call_id.0.clone(),
                    ok: true,
                    output,
                    error,
                });
            }
            SessionEvent::TurnEnd { ts, .. } => {
                // 轮界收束（v2.101）：映成带时刻的 TurnDone——重放据此冻结
                // 计时行读数。steps 日志未记录 = 不带（宁缺勿假）；旧日志
                // 无此事件，重放行为不变。
                out.push(AgentEvent::TurnDone {
                    session_id: session_id.into(),
                    steps: None,
                    ts: (*ts != 0).then_some(*ts),
                });
            }
            _ => {}
        }
    }
    out
}
