//! 会话导出为 Markdown 与它的标题规则：转录数据走 `runner::project_events`
//! 同一投影（与转录、历史重放同源），标题三层优先（旁车 → 首条用户消息截断 →
//! 会话 id）。两个 `pub(crate)` 纯函数（`export_title` / `first_user_headline`）
//! 抽出来就是为了能被用例钉住——命令本体要 Tauri `State`，测试打不进去（台账 T6）。

use tauri::State;

use session_store::{load_session, session_file, SessionEvent};

use crate::runner;
use crate::state::AppState;

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
    // 台账 T6：标题**只问这一个会话**——以前这里为了拿一条标题调
    // `session_log::list(整个 sessions/)`，等于每导出一次就把所有会话的日志
    // 逐行扫一遍、旁车逐个读一遍再按 mtime 排序（列表命令的活儿）。
    // 现在旁车优先、缺则用本会话日志里首条用户消息现算，规则与
    // `persist.rs::scan_session_file` 同源（同一个 `SESSION_TITLE_CHARS`）。
    let (events, first_user) = match held {
        Some(lock) => {
            let session = lock.lock().await;
            (
                runner::project_events(session.events(), &session_id),
                first_user_headline(session.events()),
            )
        }
        None => {
            let dir = &state.sessions_dir;
            let log = session_file(dir, &session_id).map_err(|e| format!("非法会话 id: {e}"))?;
            let (session, _) = load_session(&log).map_err(|e| format!("会话日志读取失败: {e}"))?;
            (
                runner::project_events(session.events(), &session_id),
                first_user_headline(session.events()),
            )
        }
    };
    let title = export_title(
        session_store::read_meta(&state.sessions_dir, &session_id).and_then(|m| m.title),
        first_user,
        &session_id,
    );
    let md = crate::export::render_markdown(&title, &session_id, &events);
    std::fs::write(&path, md).map_err(|e| format!("写入失败 {path}: {e}"))?;
    Ok(path)
}

/// 导出的标题三层优先：旁车重命名标题 → 本会话首条用户消息截断 → 会话 id。
/// 与列表层（`persist.rs::list_sessions`）同一条次序，差别只在"只算这一个会话"。
/// 抽成纯函数是为了能被用例钉住——命令本体要 Tauri `State`，测试打不进去。
pub(crate) fn export_title(
    sidecar: Option<String>,
    first_user: Option<String>,
    session_id: &str,
) -> String {
    sidecar
        .filter(|t| !t.trim().is_empty())
        .or(first_user)
        .unwrap_or_else(|| session_id.to_string())
}

/// 列表层自动标题的本会话版本：首条用户消息前 `SESSION_TITLE_CHARS` 字。
/// 宽度常量从 session_store 取，**不在这里重抄 40**（同一读数两个出处必漂）。
pub(crate) fn first_user_headline(events: &[SessionEvent]) -> Option<String> {
    events.iter().find_map(|e| match e {
        SessionEvent::UserMessage { content, .. } => Some(
            content
                .chars()
                .take(session_store::SESSION_TITLE_CHARS)
                .collect(),
        ),
        _ => None,
    })
}
