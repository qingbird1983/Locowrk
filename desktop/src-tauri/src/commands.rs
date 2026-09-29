//! Tauri 命令薄适配层（DESIGN.md §10.3 / §12.1）：只做参数转换与状态
//! 存取，业务逻辑在 runner / kernel，本文件不写一行。

use tauri::State;

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
        &state.sink(),
    )
    .await
}
