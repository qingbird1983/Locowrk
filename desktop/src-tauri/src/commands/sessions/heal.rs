//! 会话回退与崩溃后裁决（两条"改写下一轮请求形状"的收束路径）：编辑重发的截断步
//! （v2.57 §3.34，rewind + fork + 镜像重写）与悬空调用的事实认定（G2 §5，只写一次
//! 并实时回推投影）。两者都要会话锁，所以都先用 try_lock 忙守卫拒绝运行中的轮；
//! 拒答骨架统一从 `lifecycle::busy_guard_msg` 出（T28 并处，夹具钉文案），
//! 动作词各说各的事。

use tauri::State;

use session_store::{SessionEvent, UncertainDecision};

use crate::events::AgentEvent;
use crate::state::AppState;
use crate::timeutil::now_ms;

/// 消息编辑重发的截断步（v2.57 §3.34）：把日志截到 `pos` 之前（rewind_to
/// + fork，⑨ 先例）并物理同步镜像（persist rewrite，`.undo.bak` 留痕同
/// ⑲）。`pos = None` = 最后一条用户消息（本进程实时发送的消息无事件位，
/// 由后端解析）。忙锁 try_lock 守卫：运行中拒绝（前端编辑按钮同规禁用）；
/// 重发由前端走既有 `agent_chat`（审批/用量/提取零新链路）。
#[tauri::command]
pub async fn session_truncate_after(
    session_id: String,
    pos: Option<usize>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let held = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "会话不存在（还没有任何消息）".to_string())?;
    if held.try_lock().is_err() {
        return Err(super::lifecycle::busy_guard_msg(
            "编辑重发",
            super::lifecycle::BUSY_TAIL_WAIT_TURN,
        ));
    }
    let mut session = held.lock().await;
    crate::session_log::truncate_after(&mut session, pos)?;
    crate::session_log::rewrite_after_undo(&state.sessions_dir, &session_id, &mut session);
    Ok(())
}

/// 崩溃恢复裁决（G2，可靠性稿 §5）：用户对 uncertain 悬空调用给出事实
/// 认定（视为已完成 / 视为未执行 / 检查后记录），落 `ToolExecutionUncertain`
/// 事件（只写一次，重放/重启不再询问）并实时回推投影（UncertainResolved
/// 收束决策卡 + 合成 ToolResult 按裁决闭合工具行）。忙锁 try_lock 同
/// 编辑重发：运行中拒绝——裁决改写下一轮的请求形状，与进行中的轮互斥。
/// 日志不回填 ToolResult：悬空是日志事实，闭合是投影层的合成。
#[tauri::command]
pub async fn session_resolve_uncertain(
    session_id: String,
    call_id: String,
    decision: String,
    note: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let held = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "会话不存在（还没有任何消息）".to_string())?;
    if held.try_lock().is_err() {
        return Err(super::lifecycle::busy_guard_msg(
            "裁决",
            super::lifecycle::BUSY_TAIL_WAIT_TURN,
        ));
    }
    let dec =
        UncertainDecision::parse(&decision).ok_or_else(|| format!("未知裁决类型: {decision}"))?;
    let note = note.unwrap_or_default();
    if matches!(dec, UncertainDecision::Inspected) && note.trim().is_empty() {
        return Err("「检查后记录」需要填写检查结果".into());
    }
    let mut session = held.lock().await;
    let scan = session_store::scan_uncertain(session.events());
    if !scan.dangling.contains(call_id.as_str()) {
        return Err("该调用已有结果，不是待裁决的悬空调用".into());
    }
    if scan.decisions.contains_key(call_id.as_str()) {
        return Err("该调用已经裁决过了".into());
    }
    let ts = now_ms();
    session.append(SessionEvent::ToolExecutionUncertain {
        call_id: plugin_api::CallId(call_id.clone()),
        decision: dec,
        note: note.trim().to_string(),
        ts,
    });
    crate::session_log::persist_pending(&state.sessions_dir, &session_id, &mut session);
    // 实时回推（与重放同形）：卡收束 + 行闭合。tool 从 ToolCall 事件
    // 找回；声明位缺失回落 call_id（ApprovalResolved 同款）。
    let tool = session
        .events()
        .iter()
        .find_map(|e| match e {
            SessionEvent::ToolCall {
                call_id: c, tool, ..
            } if c.0 == call_id => Some(tool.clone()),
            _ => None,
        })
        .unwrap_or_else(|| call_id.clone());
    state.sink.emit(&AgentEvent::UncertainResolved {
        session_id: session_id.clone(),
        call_id: call_id.clone(),
        tool: tool.clone(),
        decision: dec.as_str().into(),
        note: note.trim().to_string(),
    });
    let (output, error) = session_store::closure(Some((dec, note.trim())));
    state.sink.emit(&AgentEvent::ToolResult {
        session_id,
        call_id,
        ok: true,
        output,
        error,
    });
    Ok(())
}
