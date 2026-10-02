//! 会话运营命令（v2.49，UI shell 文档 §3.6 行尾 `…` 菜单的后端）：重命名
//! / 删除 / 搜索 / 导出。薄适配层——文件面在 session_store::persist（经
//! session_log 薄封装），投影与渲染在 runner / export，这里只接参数。

use tauri::State;

use session_store::SessionSearchHit;

use crate::state::AppState;

/// 会话重命名：写旁车 `<id>.meta.json` 标题（trim 后空 = 回落自动标题）。
/// 内存会话不动，列表层生效。
#[tauri::command]
pub async fn session_rename(
    session_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    crate::session_log::rename(&state.sessions_dir, &session_id, &title)
        .map_err(|e| format!("重命名失败: {e}"))
}

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
    Ok(crate::session_log::search(
        &state.sessions_dir,
        &query,
        limit.unwrap_or(50).min(200),
    ))
}

/// 会话导出为 Markdown 转录（`pick_save_path` 选定路径后调用）：数据 =
/// 会话日志同一投影（内存优先，缺则回读磁盘），与转录/历史重放同源。
/// 返回写出路径。
#[tauri::command]
pub async fn session_export(
    session_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // std 锁卫语句先行落变量（match 暂挂临时会横跨 await，非 Send）
    let held = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned();
    let events = match held {
        Some(lock) => {
            let session = lock.lock().await;
            crate::runner::project_events(session.events(), &session_id)
        }
        None => {
            let dir = &state.sessions_dir;
            let log = session_store::session_file(dir, &session_id)
                .map_err(|e| format!("非法会话 id: {e}"))?;
            let (session, _) =
                session_store::load_session(&log).map_err(|e| format!("会话日志读取失败: {e}"))?;
            crate::runner::project_events(session.events(), &session_id)
        }
    };
    let title = crate::session_log::list(&state.sessions_dir)
        .into_iter()
        .find(|m| m.id == session_id)
        .and_then(|m| m.title)
        .unwrap_or_else(|| session_id.clone());
    let md = crate::export::render_markdown(&title, &session_id, &events);
    std::fs::write(&path, md).map_err(|e| format!("写入失败 {path}: {e}"))?;
    Ok(path)
}
