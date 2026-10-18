//! 会话删除与正文搜索：删除带忙锁守卫（运行中的会话默认拒删，开发者开关放行
//! 才强删，删后要剔内存条目与"最近聊过"指针）；搜索是全目录同步 IO ⇒ 会话命令里
//! **唯一**走 `spawn_blocking` 的一条，其余单文件小 IO 包一层是无收益的形式主义。

use tauri::State;

use session_store::SessionSearchHit;

use crate::state::AppState;

/// 会话删除：忙锁守卫——运行中的会话锁被整轮持有，默认拒绝；开发者开关
/// `developer.allowDeleteRunningSession`（后端直读设置，不信前端转述）
/// 放行时强制删（运行轮余段会重建日志文件）。删除后同步剔除内存条目与
/// 调度"最近聊过"指针（防僵尸注入轮）。幂等：不存在也是 Ok。
#[tauri::command]
pub async fn session_delete(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let lock = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned();
    if let Some(l) = lock {
        if l.try_lock().is_err() && !state.settings.developer().allow_delete_running_session {
            return Err(
                "会话正在运行，暂不能删除（稍后再试，或在 设置 → 开发者 允许强制删除）".into(),
            );
        }
        state
            .sessions
            .lock()
            .expect("sessions poisoned")
            .remove(&session_id);
    }
    crate::session_log::delete(&state.sessions_dir, &session_id)
        .map_err(|e| format!("删除失败: {e}"))?;
    state.forget_chat(&session_id);
    Ok(())
}

/// 会话正文搜索（标题仍是前端过滤）：UserMessage / AssistantMessage
/// 大小写不敏感子串扫描，mtime 降序；limit 缺省 50、上限 200。
#[tauri::command]
pub async fn session_search(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionSearchHit>, String> {
    // 批 F-24b / M-18f：这是会话面**唯一** O(全目录) 的同步 IO（逐文件读 jsonl 扫正文），
    // 改前直接跑在 async 线程上 = 卡 IPC 运行时，故包 `spawn_blocking`。
    // 其余会话命令（rename / associate / delete / list / truncate）都是单文件小 IO，
    // 给它们包一层是**无收益的形式主义** ⇒ 不改，口径写进 ROADMAP 边界。
    // 另：审计给的「`session_history` 锁内取快照、锁外投影」实测**不成立**——
    // `Session::events()` 返回借用会话锁守卫的切片，锁外投影只能先 `to_vec()` 深克隆，
    // 比原地投影更贵 ⇒ 那条修法本批不采纳（如实记档，不改口径当修复）。
    let dir = state.sessions_dir.clone();
    tokio::task::spawn_blocking(move || {
        crate::session_log::search(&dir, &query, limit.unwrap_or(50).min(200))
    })
    .await
    .map_err(|e| format!("会话搜索任务被线程池拒绝: {e}"))
}
