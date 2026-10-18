//! 应用内通知的读 / 置读 / 清空（v2.51 §3.13）：薄适配层，条目存储本体在 notifications。

use tauri::State;

use crate::state::AppState;

// ---- 应用内通知（v2.51 §3.13）：读 / 全读 / 单读 / 清空，薄适配层 ----

#[tauri::command]
pub async fn notification_list(
    state: State<'_, AppState>,
) -> Result<Vec<crate::notifications::NotificationItem>, String> {
    Ok(state.notifications.list())
}

#[tauri::command]
pub async fn notification_read_all(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.notifications.mark_all_read())
}

/// 单条置读（行点击跳会话时），返回是否命中。
#[tauri::command]
pub async fn notification_read(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.notifications.mark_read(&id))
}

#[tauri::command]
pub async fn notification_clear(state: State<'_, AppState>) -> Result<(), String> {
    state.notifications.clear();
    Ok(())
}
