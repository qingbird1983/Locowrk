//! 聊天轮次主循环（§12.1 `agent_chat` 的业务所在：命令层保持薄适配）。
//!
//! 每轮：用户消息进事件日志 → Kernel::step 循环（ReAct，§7.2）→
//! 把日志新增段投影成 `agent://` 事件。UI 事件是会话日志的投影，
//! 不是第二真相源——阶段六的回放/恢复可复用同一投影。
//!
//! 失败约定：内核报错 / 超出步数上限时不发 TurnDone，直接返回 Err
//! （前端以 invoke 的 reject 收尾），避免与成功路径的事件重复。

use kernel_agent::Kernel;
use plugin_api::SideEffect;
use session_store::{Session, SessionEvent};

use crate::events::{AgentEvent, EventSink};

/// 单轮 step 上限：防止模型反复发工具调用打转（防御性兜底）。
pub const MAX_STEPS: u32 = 8;

/// 跑完一轮对话，返回实际 step 数。
pub async fn run_turn(
    kernel: &Kernel,
    session: &mut Session,
    session_id: &str,
    message: String,
    sink: &dyn EventSink,
) -> Result<u32, String> {
    session.append(SessionEvent::UserMessage { content: message });

    for steps in 1..=MAX_STEPS {
        let before = session.len();
        if let Err(e) = kernel.step(session).await {
            return Err(format!("kernel step: {e}"));
        }
        project_new_events(session, before, session_id, sink);

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
fn project_new_events(session: &Session, from: usize, session_id: &str, sink: &dyn EventSink) {
    for e in &session.events()[from..] {
        match e {
            SessionEvent::AssistantMessage { content, .. } => {
                // 纯工具调用的 assistant 步正文为空，不发空 chunk
                if !content.is_empty() {
                    sink.emit(&AgentEvent::Chunk {
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
            } => sink.emit(&AgentEvent::ToolCall {
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
            } => sink.emit(&AgentEvent::ToolResult {
                session_id: session_id.into(),
                call_id: call_id.0.clone(),
                ok: error.is_none(),
                output: output.clone(),
                error: error.clone(),
            }),
            _ => {}
        }
    }
}

fn side_effect_str(se: &SideEffect) -> &'static str {
    match se {
        SideEffect::Pure => "pure",
        SideEffect::Compensable { .. } => "compensable",
        SideEffect::Irreversible => "irreversible",
    }
}
