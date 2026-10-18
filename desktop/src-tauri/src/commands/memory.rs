//! 记忆管理命令（⑪ 记忆管理页）：薄适配层，读源 = 真源 md（S1 刀②b-1 起管理页
//! 列举与过滤不再发 SQL；`memory::MemoryStore` 里剩下的 SQL 只碰影子表那一侧）。
//! 来源徽章 / 时间文案由前端按结构化字段渲染（§8.1）。

use tauri::State;

use crate::memory::MemoryRow;
use crate::state::AppState;

// ---- 记忆管理命令（⑪，Context and Memory.md §7.2）：薄适配层，读源 = 真源 md，
// 取数全在 memory::MemoryStore（影子表那一侧的 SQL 不从这里走）。来源徽章 / 时间文案
// 由前端按结构化字段渲染（§8.1）。 ----

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

/// 队列 Q15（MB §5 · 看板 T2）：打开**该项目**的人类层记忆目录
/// （`<数据根>/memory/projects/<可读名-哈希>/`，tree.md 与 facts/ 都在这层，
/// 向上一级即机器投影目录 `memory-tree/`——机器向 JSON 不设按钮，稿 §5 原话）。
/// 不存在先建再开：首次点击也落对位置，不给用户一个"打开失败"的空目录。
///
/// ⚠ 入参是 **workspace 串**而不是路径，路径由 `MemoryFiles::project_dir` 单点
/// 算出——这比 `reveal_in_explorer` 那条「必须是 fsRoots 之一」的白名单更窄：
/// 这里连"前端递一个路径"这件事都不存在，`..` / 绝对路径 / 任意串都只能落进
/// 安全化 + fnv 短哈希之后的那一个目录名里。`''`（未归属）**报错而不是给目录**：
/// 未归属不是项目，它的 tree.md 在 `_unaffiliated/`、事实在 `global/`。
/// 这两条性质的承重用例 = `plugin_api::memlayout::tests::project_dir_rejects_unowned_and_is_the_single_formula`
/// 与 `::project_dir_keeps_any_workspace_string_under_the_root`，句柄转发那一步由
/// `memory_files::tests::project_dir_is_the_handle_view_of_the_same_formula_and_holds_tree_md`
/// 钉（命令本体是薄壳，与 `asset_reveal` 同例——`State<'_, AppState>` 不在单测可构造
/// 面内，逻辑一律归句柄；`tmp/q15-shape.mjs` 的 E 面逐条核这三个名字真在场）。
#[tauri::command]
pub async fn memory_reveal_project(
    workspace: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dir = state.memory_files.project_dir(&workspace)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("记忆项目目录创建失败 {}: {e}", dir.display()))?;
    super::sessions::open_in_explorer(&dir.to_string_lossy())
}

/// 队列 Q10（L4）：记忆注入的**只读解释面板**数据源——最近一次"真算过四信号"的
/// 注入回执 + 观测表的三个派生读数。薄适配层：读盘、筛行、解析、降级全在
/// `crate::inject_view`（那里有两条口径：只找有回执的行、坏字节不上抛）。
#[tauri::command]
pub async fn memory_inject_view(
    state: State<'_, AppState>,
) -> Result<crate::inject_view::InjectView, String> {
    crate::inject_view::view(&state.recall_log)
}
