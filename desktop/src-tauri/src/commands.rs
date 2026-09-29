//! Tauri 命令薄适配层（DESIGN.md §10.3 / §12.1）：只做参数转换与状态
//! 存取，业务逻辑在 runner / approval / kernel，本文件不写一行。

use tauri::State;

use crate::events::ApprovalRequestPayload;
use crate::runner;
use crate::state::AppState;

/// 聊天命令。轮次内容全部经 `agent://` 事件推送（§12.2），返回值只表达
/// 轮次是否成功收束（返回实际 step 数）。
#[tauri::command]
pub async fn agent_chat(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    runner::run_turn(
        &state.kernel,
        &mut session,
        &session_id,
        message,
        state.sink.as_ref(),
    )
    .await
}

/// 审批决策（§12.1）：回填 `agent://approval-request` 对应的请求。
/// 与会话锁无交集，审批弹窗打开时聊天通道仍可安全阻塞等待。
#[tauri::command]
pub async fn agent_approve(
    request_id: String,
    allowed: bool,
    reason: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.approval.decide(&request_id, allowed, reason)
}

/// 未决审批列表（§13.1 审批中心）：前端刷新/重连时补拉。
/// （Tauri 约定：async 命令含引用入参必须返回 Result，这里恒为 Ok。）
#[tauri::command]
pub async fn list_pending_approvals(
    state: State<'_, AppState>,
) -> Result<Vec<ApprovalRequestPayload>, String> {
    Ok(state.approval.pending())
}
