//! 子 Agent 转录读取（§3.16，v2.52）：`sub-transcripts/<child_id>.jsonl` 走与主会话
//! `session_history` **同一**条 `project_events` 全量投影 ⇒ 前端页签的数据形状与主转录
//! 一致。文件缺席 = Err（页签如实报不可用），坏行跳过的条数落 warn。

use tauri::State;

use session_store::{load_session, session_file};

use crate::events::AgentEvent;
use crate::runner;
use crate::state::AppState;

/// 子 Agent 转录读取（§3.16，v2.52）：`sub-transcripts/<child_id>.jsonl`
/// load 后走与 `session_history` **同一** `project_events` 全量投影——
/// 前端页签的数据形状与主转录一致。文件缺席 = Err（页签如实报不可用）。
#[tauri::command]
pub async fn subagent_transcript(
    child_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let log = session_file(&state.sub_transcripts_dir, &child_id)
        .map_err(|e| format!("非法子会话 id: {e}"))?;
    if !log.exists() {
        return Err(format!("子转录不存在（未落盘或已清理）: {child_id}"));
    }
    let (session, skipped) = load_session(&log).map_err(|e| format!("子转录读取失败: {e}"))?;
    if skipped > 0 {
        tracing::warn!(child = %child_id, skipped, "sub transcript 坏行跳过");
    }
    Ok(runner::project_events(session.events(), &child_id))
}
