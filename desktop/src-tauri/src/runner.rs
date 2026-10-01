//! 聊天轮次主循环（§12.1 `agent_chat` 的业务所在：命令层保持薄适配）。
//!
//! 每轮：用户消息进事件日志 → Kernel::step 循环（ReAct，§7.2）→
//! 把日志新增段投影成 `agent://` 事件。UI 事件是会话日志的投影，
//! 不是第二真相源——阶段六的回放/恢复可复用同一投影。
//! ⑨ 起每个 step 后补推 `agent://context-usage`（进度条数据源，
//! 与压缩判定同一估算口径）。
//!
//! 失败约定：内核报错 / 超出步数上限时不发 TurnDone，直接返回 Err
//! （前端以 invoke 的 reject 收尾），避免与成功路径的事件重复。

use std::collections::HashMap;
use std::path::Path;

use kernel_agent::{estimate_tokens, Kernel};
use session_store::{Session, SessionEvent};

use crate::session_log;
use crate::usage::UsageRecorder;

use crate::events::{side_effect_str, AgentEvent, EventSink};

/// 单轮 step 上限：防止模型反复发工具调用打转（防御性兜底）。
pub const MAX_STEPS: u32 = 8;

/// 进度条口径（⑨）：threshold = 窗口 × 百分比（组装点算好），
/// window_tokens 未知传 0（前端按"未设置"处理）。
#[derive(Debug, Clone, Copy)]
pub struct UsagePolicy {
    pub threshold_tokens: u64,
    pub window_tokens: u64,
}

/// 跑完一轮对话，返回实际 step 数。`stats` 是 ⑩③ 使用统计写入端
/// （usage.sqlite3，None = 内置替身测试）。`log` 是 ⑲ 会话落盘目录
/// （`<数据根>/sessions`，None = 不落盘，测试路径）——磁盘是日志的镜像，
/// 用户消息与每个 step 的新增段与投影共用同一游标同步追加。
#[allow(clippy::too_many_arguments)]
pub async fn run_turn(
    kernel: &Kernel,
    session: &mut Session,
    session_id: &str,
    message: String,
    usage: Option<UsagePolicy>,
    sink: &dyn EventSink,
    stats: Option<&UsageRecorder>,
    log: Option<&Path>,
) -> Result<u32, String> {
    let log_start = session.len();
    session.append(SessionEvent::UserMessage { content: message });
    if let Some(dir) = log {
        session_log::append_slice(dir, session_id, session, log_start);
    }
    if let Some(u) = usage {
        emit_usage(session, session_id, u, sink);
    }

    for steps in 1..=MAX_STEPS {
        let before = session.len();
        // ⑭：带会话身份 step——CallCtx.session_id 供内置调度工具绑定
        // 创建会话（§5.2 规划字段；普通 step 保持 None 语义不变）
        let streamed = match kernel.step_as(session, session_id).await {
            // ⑳：streamed = 本步正文已经 delta 直推（ctx 里 LlmStreamSink
            // 在册），投影时抑制整段 chunk 防重复
            Ok(kernel_agent::StepOutcome::Continue { streamed }) => streamed,
            Err(e) => return Err(format!("kernel step: {e}")),
        };
        project_new_events(session, before, session_id, sink, streamed);
        if let Some(dir) = log {
            session_log::append_slice(dir, session_id, session, before);
        }
        record_stats(session, before, stats);
        if let Some(u) = usage {
            emit_usage(session, session_id, u, sink);
        }

        // 收敛判据：本步 assistant 没有再发工具调用 → 轮次结束
        let done = matches!(
            session.events().last(),
            Some(SessionEvent::AssistantMessage { tool_calls, .. }) if tool_calls.is_empty()
        );
        if done {
            sink.emit(&AgentEvent::TurnDone {
                session_id: session_id.into(),
                steps,
            });
            return Ok(steps);
        }
    }

    Err(format!("超过单轮步数上限（{MAX_STEPS} 步），强制收束"))
}

/// 把日志新增段 `[from..]` 投影成 UI 事件（顺序 = 日志顺序）。
/// `suppress_assistant_chunk`（⑳）：本步正文已经流式 delta 直推时跳过
/// 整段 chunk，避免同一正文出现两遍。
fn project_new_events(
    session: &Session,
    from: usize,
    session_id: &str,
    sink: &dyn EventSink,
    suppress_assistant_chunk: bool,
) {
    for e in project_events(&session.events()[from..], session_id) {
        if suppress_assistant_chunk && matches!(e, AgentEvent::Chunk { .. }) {
            continue;
        }
        sink.emit(&e);
    }
}

/// 事件切片 → UI 事件序列（纯函数）。增量投影（run_turn）与 ⑲ 全量恢复
/// 投影（session_history 命令）共用同一映射——"UI 事件是会话日志的投影"
/// 的单点实现。
pub fn project_events(events: &[SessionEvent], session_id: &str) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    for e in events {
        match e {
            SessionEvent::AssistantMessage { content, .. } => {
                // 纯工具调用的 assistant 步正文为空，不发空 chunk
                if !content.is_empty() {
                    out.push(AgentEvent::Chunk {
                        session_id: session_id.into(),
                        delta: content.clone(),
                    });
                }
            }
            SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                side_effect,
                plugin_version,
                ..
            } => out.push(AgentEvent::ToolCall {
                session_id: session_id.into(),
                call_id: call_id.0.clone(),
                tool: tool.clone(),
                args: args.clone(),
                side_effect: side_effect_str(side_effect).into(),
                plugin_version: plugin_version.to_string(),
            }),
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
            _ => {}
        }
    }
    out
}

/// ⑩③ 使用统计（exposure 文档 §3.3）：ToolResult 闭合时按 (tool,
/// plugin_id) 记一次成败——ToolCall 先行落日志（纪律 5），被拒/悬空调用
/// 不计成功。异步通道 + 5s 批量刷盘在 UsageRecorder 内部。
fn record_stats(session: &Session, from: usize, stats: Option<&UsageRecorder>) {
    let Some(stats) = stats else { return };
    let mut pending: HashMap<String, (String, Option<String>)> = HashMap::new();
    for e in &session.events()[from..] {
        match e {
            SessionEvent::ToolCall {
                call_id,
                tool,
                plugin_id,
                ..
            } => {
                pending.insert(call_id.0.clone(), (tool.clone(), plugin_id.clone()));
            }
            SessionEvent::ToolResult { call_id, error, .. } => {
                if let Some((tool, plugin_id)) = pending.remove(&call_id.0) {
                    stats.record(&tool, plugin_id.as_deref(), error.is_none());
                }
            }
            _ => {}
        }
    }
}

/// 进度条口径与压缩判定同源：折叠后的消息用 kernel_agent::estimate_tokens。
fn emit_usage(session: &Session, session_id: &str, u: UsagePolicy, sink: &dyn EventSink) {
    sink.emit(&AgentEvent::ContextUsage {
        session_id: session_id.into(),
        used: estimate_tokens(&session.derive_messages()),
        threshold: u.threshold_tokens,
        window_tokens: u.window_tokens,
    });
}
