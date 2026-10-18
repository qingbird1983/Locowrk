//! 记忆人类层文件（F1，《Memory File-ization Design v1.1》）：宿主侧
//! `~/.Locowrk/memory/` 的读写编排——事实 md 转发给共享布局
//! （plugin_api::memlayout，插件与宿主同一份实现），tree.md 渲染、
//! **人改保护**与**启动导出投影**归本模块。
//!
//! 纪律（S1 刀②c-2 后）：这些 md **就是真源**，不是任何库的镜像。写事实 = 先落
//! 文件、失败上抛（见 `memory::crud`）；本模块内仍"失败只 warn"的只剩投影
//! `tree.md`——它是可重建的派生物，不是记忆本身。sqlite 侧在本模块只保留
//! `reconcile_*`（文件 → 影子单向对齐）与 `export_rows`（对账读影子），
//! **"影子 → 文件"方向的写动作已全部停止**（判定矩阵见 `memory_reconcile` 头注）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use memory_tree::{MemoryTree, TreeStore};

use plugin_api::memlayout::{self, FactEntry};

use crate::memory::MemoryStore;
use crate::timeutil::to_epoch_ms;

/// tree.md 一节标题（L0）宽度。
const TREE_TITLE_CHARS: usize = 60;

/// md 层句柄（root = `<数据根>/memory`）。纯路径持有者，便宜可克隆。
#[derive(Debug, Clone)]
pub struct MemoryFiles {
    root: PathBuf,
}

impl MemoryFiles {
    pub fn new(data_root: &Path) -> Self {
        Self {
            root: data_root.join("memory"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 写一条事实 md（转发共享布局）。收 [`FactEntry`] 而不是六个平铺参数：
    /// 那个结构体是**唯一**的渲染输入，将来再加 frontmatter 字段（刀③ 就加了一个 `ty`）
    /// 不必把每一层签名都跟着改一遍——平铺参数的那次改动牵动了这里和两个调用点，
    /// 收成一条结构体引用之后，同一次改动的宿主侧落点只剩 `store.rs::write_md`。
    pub fn write_fact(&self, e: &FactEntry) -> Result<PathBuf, String> {
        memlayout::write_fact(&self.root, e)
    }

    /// **软删**一条事实 md（S1 刀④a · AC-S1-7）：搬进 `obsolete/`，索引跟着刷。
    /// `false` = 这条本来不在。名字里带 `soft_` 是刻意的——这片里曾经只有一个
    /// `delete_fact`，它直接 `remove_file`；真源 = 文件之后"删掉"就成了不可逆动作，
    /// 而界面上的删除按钮不该承担不可逆（真删只发生在清空回收区那一次，且要确认）。
    /// 硬删出口现在是 `memlayout::purge_obsolete`，全生产面上对 `delete_fact` /
    /// `clear_facts` 的调用点必须恒 0（闸 = `SS15`）。
    pub fn soft_delete_fact(&self, workspace: &str, key: &str) -> Result<bool, String> {
        memlayout::soft_delete_fact(&self.root, workspace, key)
    }

    /// 该项目的人类层家目录（队列 Q15 · `memory_reveal_project` 与后续级联删除
    /// 的**唯一**路径来源）。`''` = 上抛 Err：未归属不是项目，不给它目录。
    pub fn project_dir(&self, workspace: &str) -> Result<PathBuf, String> {
        memlayout::project_dir(&self.root, workspace)
    }

    /// 项目 tree.md 落点。未归属（''）不是项目——固定 `_unaffiliated`
    /// 文件夹，不落进伪"项目-哈希"目录（名字与"住在 projects 树下"都归
    /// memlayout 一处定义，这里只引用）。
    fn tree_md_path(&self, workspace: &str) -> Result<PathBuf, String> {
        let dir = if workspace.is_empty() {
            memlayout::unaffiliated_dir(&self.root)
        } else {
            memlayout::project_dir(&self.root, workspace)?
        };
        Ok(dir.join("tree.md"))
    }

    /// 写 tree.md（**人改保护**：文件 mtime 新于全部投影 mtime = 人在
    /// 整理之后动过文件——F2 reconcile 尚未落地前不静默覆盖，宁缺勿假）。
    /// 返回是否真的写入。
    pub fn write_tree_md(
        &self,
        workspace: &str,
        content: &str,
        newest_projection_ms: i64,
    ) -> Result<bool, String> {
        let path = self.tree_md_path(workspace)?;
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(t) = meta.modified() {
                let md_ms = to_epoch_ms(t);
                if md_ms > newest_projection_ms {
                    return Ok(false);
                }
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("memory 目录创建失败 {}: {e}", dir.display()))?;
        }
        std::fs::write(&path, content)
            .map_err(|e| format!("tree.md 写入失败 {}: {e}", path.display()))?;
        Ok(true)
    }
}

/// tree.md 渲染（设计 §2.1：每章节一节 + 摘要 + 隐性指针注释；会话小节
/// 分组）。会话 = `##`、章节 = `###`（章节号数值序——BTreeMap 字典序会把
/// b10 排到 b2 前，M3 同纪律）。占位章节不进人读层（宁缺勿假）。
pub fn render_tree_md(workspace: &str, sessions: &[(String, String, &MemoryTree)]) -> String {
    let heading = if workspace.is_empty() {
        "未归属会话".to_string()
    } else {
        memlayout::project_dir_name(workspace)
    };
    let mut out = String::new();
    out.push_str(&format!(
        "# 记忆树 · {}\n\n> 由整理系统生成的人读层；原文不进本文件——按事件区间回会话日志查看。\n",
        heading
    ));
    for (sid, title, tree) in sessions {
        let mut chapters: Vec<&memory_tree::MemoryChapter> =
            tree.chapters.values().filter(|b| !b.pending).collect();
        if chapters.is_empty() {
            continue;
        }
        chapters.sort_by_key(|b| {
            b.chapter_id
                .0
                .strip_prefix('b')
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        });
        out.push_str(&format!(
            "\n## {}\n",
            memlayout::one_line(
                if title.trim().is_empty() { sid } else { title },
                TREE_TITLE_CHARS
            )
        ));
        // 轮级总结（Organize Summary Design v1 §3.5）：以引用块渲染在
        // 该轮章节组正上方（字面意义的「标题上层」）。行间区（小节之外）
        // 的 `>` 行不进 memory_tree_reconcile 的小节收集（cur=None 丢弃），
        // 人改保护纪律不变。文本内的 md 结构字符按行前缀隔离。
        for s in &tree.summaries {
            out.push_str(&format!(
                "\n> 📋 **段落总结**（事件 {}-{}）\n",
                s.from, s.to
            ));
            for line in s.text.trim().lines() {
                out.push_str(&format!("> {}\n", line.trim()));
            }
            out.push('\n');
        }
        for b in chapters {
            out.push_str(&format!(
                "\n### {} · {}\n\n",
                b.chapter_id.0,
                b.title.trim()
            ));
            out.push_str(b.summary.trim());
            out.push_str("\n\n");
            out.push_str(&format!(
                "<!-- range: {}-{} · source: {} · session: {} -->\n",
                b.event_range.0, b.event_range.1, b.source, sid
            ));
        }
    }
    out
}

/// 单工作区同步：读投影 + 旁车标题 → 渲染 → 带人改保护落盘。返回是否
/// 真的写入。全部树都无实章节（影子期尾巴会话）= 跳过。
pub fn sync_workspace_tree_md(
    files: &MemoryFiles,
    store: &TreeStore,
    sessions_dir: &Path,
    workspace: &str,
) -> Result<bool, String> {
    let rows = store.scan_workspace(workspace);
    let has_real_chapter = rows
        .iter()
        .any(|(t, _, _)| t.chapters.values().any(|b| !b.pending));
    if !has_real_chapter {
        return Ok(false);
    }
    let titles: std::collections::HashMap<String, String> = crate::session_log::list(sessions_dir)
        .into_iter()
        .filter_map(|m| m.title.map(|t| (m.id, t)))
        .collect();
    let refs: Vec<(String, String, &MemoryTree)> = rows
        .iter()
        .map(|(t, sid, _)| (sid.clone(), titles.get(sid).cloned().unwrap_or_default(), t))
        .collect();
    let newest = rows.iter().map(|(_, _, ms)| *ms).max().unwrap_or(0);
    let content = render_tree_md(workspace, &refs);
    files.write_tree_md(workspace, &content, newest)
}

/// 启动导出（设计 §6 F1）：**只导 tree.md**（投影，有人改保护），幂等，返回新写数。
///
/// 旧形状里还有一段"存量 sqlite 行 → md 事实文件（文件已在 = 跳过）"，S1 刀②c-2
/// 起**整段退役**：真源 = md 之后，"库里有一行而文件没有"只有两种成因——插件写库
/// 未落文件（S1 刀③b 之后插件不再开这张表，这条消失）、人已删文件（副本补回来 =
/// 复活）。作者 2026-10-18
/// 明确裁定存量库**不回填**（"免得污染，我到时会清理掉的"），所以这里一行文件都不写。
pub fn run_initial_export(
    tree_store: &TreeStore,
    files: &MemoryFiles,
    sessions_dir: &Path,
) -> Result<usize, String> {
    let mut trees = 0usize;
    let workspaces: BTreeSet<String> = tree_store
        .scan_trees()
        .into_iter()
        .map(|s| s.workspace)
        .collect();
    for ws in workspaces {
        match sync_workspace_tree_md(files, tree_store, sessions_dir, &ws) {
            Ok(true) => trees += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!("导出 tree.md（{ws}）失败: {e}"),
        }
    }
    Ok(trees)
}

/// 首启对账 + 导出入口（state 组装点调用）：后台线程 fire-and-forget；
/// 总开关关 = 什么都不做（连 md 层都不碰）。
///
/// 顺序纪律：**先 reconcile 再导投影**——reconcile 以文件为准（人删的文件把行
/// 删掉），投影只重建 `tree.md`，两者互不遮挡。事实补洞已随刀②c-2 退役，
/// 所以这里不再存在"导出把已删文件写回来"的风险。
pub fn spawn_initial_export(
    memory: std::sync::Arc<MemoryStore>,
    tree_store: TreeStore,
    files: MemoryFiles,
    sessions_dir: PathBuf,
    files_on: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    roots_on: std::sync::Arc<dyn Fn() -> Vec<String> + Send + Sync>,
) {
    std::thread::spawn(move || {
        if !files_on() {
            return;
        }
        let mut known = roots_on();
        match memory.all_workspaces() {
            Ok(ws) => known.extend(ws),
            Err(e) => tracing::warn!("记忆 reconcile 键位枚举失败: {e}"),
        }
        match crate::memory_reconcile::run(&memory, &files, &known) {
            Ok(s) if s.has_action() => tracing::info!(
                created = s.created,
                updated = s.updated,
                deleted = s.deleted,
                warnings = s.warnings,
                "记忆文件启动对账完成"
            ),
            Ok(_) => {}
            Err(e) => tracing::warn!("记忆文件启动对账失败: {e}"),
        }
        // M5：tree.md 人改对账（编辑面另一半）——先于导出跑，人改的
        // 标题/摘要回写投影后，导出的人改保护自然放行重建。
        let tree_sum = crate::memory_tree_reconcile::run(&tree_store, &files, &known);
        if tree_sum.has_action() {
            tracing::info!(
                titles = tree_sum.titles,
                summaries = tree_sum.summaries,
                warnings = tree_sum.warnings,
                "tree.md 启动对账完成"
            );
        }
        match run_initial_export(&tree_store, &files, &sessions_dir) {
            Ok(0) => {}
            Ok(t) => tracing::info!(trees = t, "记忆投影启动导出完成"),
            Err(e) => tracing::warn!("记忆投影启动导出失败: {e}"),
        }
    });
}

#[cfg(test)]
mod tests;
