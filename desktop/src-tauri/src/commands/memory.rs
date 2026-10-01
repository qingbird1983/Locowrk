//! 记忆与经历命令（⑪ 记忆管理页 / ⑬ 经历记忆页）：薄适配层，SQL 全在
//! memory::MemoryStore（memory.sqlite3 归 memory-sqlite 插件所有，宿主只
//! 做管理面访问）与 experiences::ExperienceDb（宿主自有）。来源徽章 /
//! 时间文案由前端按结构化字段渲染（§8.1）。

use tauri::State;

use crate::experiences::ExperienceRow;
use crate::memory::MemoryRow;
use crate::state::AppState;

// ---- 记忆管理命令（⑪，Context and Memory.md §7.2）：薄适配层，SQL 全在
// memory::MemoryStore（memory.sqlite3 归 memory-sqlite 插件所有，宿主只做
// 管理页访问）。来源徽章 / 时间文案由前端按结构化字段渲染（§8.1）。 ----

/// 列举记忆：query 空 = 全量（新→旧），否则 key/value/tags 子串过滤。
#[tauri::command]
pub async fn memory_list(
    query: Option<String>,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryRow>, String> {
    state
        .memory
        .list(query.as_deref().unwrap_or(""), limit.unwrap_or(200))
}

/// 编辑记忆值（只改 value，键名与来源不动；不存在报错）。
#[tauri::command]
pub async fn memory_update(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.memory.update_value(&key, &value)? {
        return Err(format!("记忆 `{key}` 不存在"));
    }
    Ok(())
}

/// 删除单条记忆（不存在报错，前端据此刷新）。
#[tauri::command]
pub async fn memory_delete(key: String, state: State<'_, AppState>) -> Result<(), String> {
    if !state.memory.delete(&key)? {
        return Err(format!("记忆 `{key}` 不存在"));
    }
    Ok(())
}

/// 清空全部记忆。返回删除条数（前端提示"已清空 n 条"）。
#[tauri::command]
pub async fn memory_clear_all(state: State<'_, AppState>) -> Result<usize, String> {
    state.memory.clear_all()
}

// ---- 经历记忆命令（⑬，Context 文档 §4.3 / §11.3"用户必须能看到并删除
// 每一条记忆"）：experiences.sqlite3 宿主自有，薄适配到 ExperienceDb。
// 语义检索的命中注入不经命令（Memory-Inject 直连），这里只服务管理页。 ----

/// 列举经历（新→旧；管理页单次拉取，不分页）。
#[tauri::command]
pub async fn experience_list(
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<ExperienceRow>, String> {
    match &state.experiences {
        Some(db) => db.list_recent(limit.unwrap_or(200).min(200)),
        None => Err("经历记忆不可用（数据库未打开）".into()),
    }
}

/// 删除一条经历（不存在报错，前端据此刷新）。
#[tauri::command]
pub async fn experience_delete(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.experiences.as_ref().ok_or("经历记忆不可用")?;
    if !db.delete(id)? {
        return Err(format!("经历 #{id} 不存在"));
    }
    Ok(())
}

/// 清空全部经历。返回删除条数。
#[tauri::command]
pub async fn experience_clear_all(state: State<'_, AppState>) -> Result<usize, String> {
    let db = state.experiences.as_ref().ok_or("经历记忆不可用")?;
    db.clear_all()
}
