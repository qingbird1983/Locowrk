//! 定时任务目的地页命令（v2.57 §3.35）：列表与删除的薄适配层。数据面在
//! scheduler::store（⑭ 宿主自有 schedule.sqlite3）；页面是读者与裁决者，
//! 不做编辑器（创建入口是模型工具 `schedule_create`）。

use serde::Serialize;
use tauri::State;

use crate::scheduler::store::Kind;
use crate::state::AppState;

/// 一条待触发调度（IPC 投影，camelCase 对齐前端 types）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleView {
    pub id: String,
    pub session_id: String,
    /// "once" | "interval"
    pub kind: &'static str,
    pub title: String,
    pub prompt: String,
    /// 下次触发时刻（unix 毫秒；逾期 = 等待会话空闲/顺延中）。
    pub run_at_ms: i64,
    pub interval_secs: Option<i64>,
    /// 忙顺延次数（引擎让位机制的真实状态，如实呈现）。
    pub deferrals: i64,
    pub created_at_ms: i64,
}

fn view(s: crate::scheduler::store::Schedule) -> ScheduleView {
    ScheduleView {
        kind: match s.kind {
            Kind::Once => "once",
            Kind::Interval => "interval",
        },
        id: s.id,
        session_id: s.session_id,
        title: s.title,
        prompt: s.prompt,
        run_at_ms: s.run_at,
        interval_secs: s.interval_secs,
        deferrals: s.deferrals,
        created_at_ms: s.created_ms,
    }
}

/// 全部待触发调度（新→旧）。库缺席 = 空表（调度降级，页面空态）。
#[tauri::command]
pub async fn schedule_list(state: State<'_, AppState>) -> Result<Vec<ScheduleView>, String> {
    match state.schedules.as_ref() {
        Some(db) => Ok(db.list_all().into_iter().map(view).collect()),
        None => Ok(Vec::new()),
    }
}

/// 删除一条调度（返回是否确有此行；一次性触发后行已被引擎删掉 = false，
/// 页面按幂等处理）。
#[tauri::command]
pub async fn schedule_delete(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    match state.schedules.as_ref() {
        Some(db) => Ok(db.delete(&id)),
        None => Ok(false),
    }
}
