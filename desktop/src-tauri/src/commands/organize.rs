//! 记忆树影子运行命令（M2，UI shell §3.36）：`/整理` 发起入口 + 开发者
//! 观测面板数据源。薄适配层——决策与管道全在 organize 模块；会话锁在
//! 命令内短暂持有（快照/判定为同步操作），整理本体后台跑不占锁。

use tauri::State;

use crate::organize::TreeDebug;
use crate::state::AppState;

/// 手动整理（`/整理` / `/organize`）：立即整理当前未整理尾巴。Err =
/// 发起失败（已有任务在途 / 没有可整理的尾巴），文案直显。
#[tauri::command]
pub async fn organize_now(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let organizer = std::sync::Arc::clone(&state.organizer);
    let session = state.session_lock(&session_id);
    let session = session.lock().await;
    organizer.organize_now(&session, &session_id)
}

/// 开发者观测面板（§3.36③）：当前会话的树投影 + 触发配置 + 尾巴现状。
/// 只读；前端打开面板时拉取，不推送不轮询（⑫ 同款）。
#[tauri::command]
pub async fn memory_tree_view(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<TreeDebug, String> {
    let session = state.session_lock(&session_id);
    let session = session.lock().await;
    Ok(state.organizer.tree_view(&session, &session_id))
}
