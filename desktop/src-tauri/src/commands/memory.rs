//! 记忆管理命令（⑪ 记忆管理页）：薄适配层，SQL 全在 memory::MemoryStore
//! （memory.sqlite3 归 memory-sqlite 插件所有，宿主只做管理面访问）。
//! 来源徽章 / 时间文案由前端按结构化字段渲染（§8.1）。

use tauri::State;

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

/// 编辑记忆值（只改 value，键名与来源不动；不存在报错）。㉘：编辑按
/// 键位 (workspace, key) 定位——workspace 来自管理页行。
#[tauri::command]
pub async fn memory_update(
    workspace: String,
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.memory.update_value(&workspace, &key, &value)? {
        return Err(format!("记忆 `{key}` 不存在"));
    }
    Ok(())
}

/// 删除单条记忆（按键位；不存在报错，前端据此刷新）。
#[tauri::command]
pub async fn memory_delete(
    workspace: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.memory.delete(&workspace, &key)? {
        return Err(format!("记忆 `{key}` 不存在"));
    }
    Ok(())
}

/// 清空全部记忆。返回删除条数（前端提示"已清空 n 条"）。
#[tauri::command]
pub async fn memory_clear_all(state: State<'_, AppState>) -> Result<usize, String> {
    state.memory.clear_all()
}

/// F2 reconcile（文件化设计 §3/§7）：diff 记忆文件 ↔ 索引，以文件为准
/// 回写。触发 = 启动（后台）+ 记忆页挂载 + 开发者手动按钮；总开关关 =
/// 空转零计数。M5 起同跑 tree.md 人改对账（编辑面另一半）——计数并入
/// scanned / updated（捆标题/摘要回写也是"文件为准的更新"）。
#[tauri::command]
pub async fn memory_reconcile(
    state: State<'_, AppState>,
) -> Result<crate::memory_reconcile::ReconcileSummary, String> {
    if !(state.memory_files_on)() {
        return Ok(Default::default());
    }
    let mut known = state.settings.fs_roots();
    known.extend(state.memory.all_workspaces()?);
    let mut sum = crate::memory_reconcile::run(&state.memory, &state.memory_files, &known)?;
    let tree = crate::memory_tree_reconcile::run(&state.tree_store, &state.memory_files, &known);
    sum.scanned += tree.scanned;
    sum.updated += tree.titles + tree.summaries;
    sum.warnings += tree.warnings;
    Ok(sum)
}

/// F2 人的编辑面入口：打开（必要时先建）记忆人类层目录。自有数据根，
/// 不经 fsRoots 白名单（那是用户仓库的防线）。
#[tauri::command]
pub async fn memory_reveal_folder(state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.memory_files.root().to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| format!("memory 目录创建失败: {e}"))?;
    super::sessions::open_in_explorer(&dir.to_string_lossy())
}
