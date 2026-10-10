//! 记忆树影子运行命令（M2，UI shell §3.36）：`/整理` 发起入口 + 开发者
//! 观测面板数据源；M3（§3.37④）只读呈现三命令（清单 / 章节清单 / 原文切片
//! ——直读投影文件，不占会话锁）。薄适配层——决策与管道全在 organize
//! 模块；会话锁只在 `organize_now` / `memory_tree_view` 内短暂持有，
//! 整理本体后台跑不占锁。

use tauri::State;

use crate::organize::{
    OriginalItem, ProjectTreeView, TreeChaptersView, TreeDebug, TreeSummaryView,
};

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

/// 树投影清单（M3 §3.37④）：记忆页会话切换器数据源（最近更新降序）。
#[tauri::command]
pub async fn memory_tree_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<TreeSummaryView>, String> {
    Ok(state.organizer.tree_sessions())
}

/// 选中会话的章节清单（M3 §3.37④）：直读投影文件，不要求会话在内存。
#[tauri::command]
pub async fn memory_tree_chapters(
    session_id: String,
    workspace: String,
    state: State<'_, AppState>,
) -> Result<TreeChaptersView, String> {
    Ok(state.organizer.tree_chapters(&session_id, &workspace))
}

/// 章节原文切片（M3 §3.37④）：与记忆管理器材料同形（build_items 同源函数）。
#[tauri::command]
pub async fn memory_tree_original(
    session_id: String,
    workspace: String,
    chapter_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<OriginalItem>, String> {
    state
        .organizer
        .tree_original(&session_id, &workspace, &chapter_id)
}

/// 项目级 L0 联合视图（M6.4 §11.4）：该项目跨会话全部捆平铺（会话标注，
/// 最近活动会话优先）+ v2.141 跨会话总结链（时间正序 = 项目进展叙事锚点，
/// Organize Summary Design v1 §3.5）。记忆页「按项目」档数据源。
#[tauri::command]
pub async fn memory_tree_project(
    workspace: String,
    state: State<'_, AppState>,
) -> Result<ProjectTreeView, String> {
    Ok(state.organizer.tree_project(&workspace))
}

/// 编辑章节标题/摘要（M5 §3.37 编辑面；直读直写投影文件，不占会话锁、
/// 不要求会话在内存）。title/summary 独立可选，只动提供的那个。
#[tauri::command]
pub async fn memory_tree_edit_chapter(
    session_id: String,
    workspace: String,
    chapter_id: String,
    title: Option<String>,
    summary: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.organizer.tree_edit_chapter(
        &workspace,
        &session_id,
        &chapter_id,
        title.as_deref(),
        summary.as_deref(),
    )
}

/// 删除章节（M5 §3.37 编辑面；两步确认在前端——后端只认结果）。
#[tauri::command]
pub async fn memory_tree_delete_chapter(
    session_id: String,
    workspace: String,
    chapter_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .organizer
        .tree_delete_chapter(&workspace, &session_id, &chapter_id)
}
