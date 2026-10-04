//! 记忆人类层文件（F1，《Memory File-ization Design v1.1》）：宿主侧
//! `~/.Locowrk/memory/` 的读写编排——事实 md 转发给共享布局
//! （plugin_api::memlayout，插件与宿主同一份实现），tree.md 渲染、
//! **人改保护**与**首启导出**归本模块。
//!
//! 纪律：md 是人读面，sqlite/投影是机器真源——本模块所有写操作都是
//! "真源已落之后的同步"，失败只 warn（降级不拦聊天）；导出幂等（已有
//! 文件一律不动，人的编辑永不被首启覆盖）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use memory_tree::{MemoryTree, TreeStore};

use plugin_api::memlayout::{self, FactEntry};

use crate::memory::MemoryStore;

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

    /// 写一条事实 md（转发共享布局）。调用方保证 sqlite 已落。
    pub fn write_fact(
        &self,
        workspace: &str,
        key: &str,
        value: &str,
        tags: &str,
        source: &str,
        updated_at: i64,
    ) -> Result<PathBuf, String> {
        memlayout::write_fact(
            &self.root,
            &FactEntry {
                workspace,
                key,
                value,
                tags,
                source,
                updated_at,
            },
        )
    }

    /// 删一条事实 md（幂等）。
    pub fn delete_fact(&self, workspace: &str, key: &str) -> Result<bool, String> {
        memlayout::delete_fact(&self.root, workspace, key)
    }

    /// 项目 tree.md 落点。未归属（''）不是项目——固定 `_unaffiliated`
    /// 文件夹，不落进伪"项目-哈希"目录。
    fn tree_md_path(&self, workspace: &str) -> PathBuf {
        let dir = if workspace.is_empty() {
            "_unaffiliated".to_string()
        } else {
            memlayout::project_dir_name(workspace)
        };
        self.root.join("projects").join(dir).join("tree.md")
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
        let path = self.tree_md_path(workspace);
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(t) = meta.modified() {
                let md_ms = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
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

/// tree.md 渲染（设计 §2.1：每捆一节 + 摘要 + 隐性指针注释；会话小节
/// 分组）。会话 = `##`、捆 = `###`（捆号数值序——BTreeMap 字典序会把
/// b10 排到 b2 前，M3 同纪律）。占位捆不进人读层（宁缺勿假）。
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
        let mut bundles: Vec<&memory_tree::MemoryBundle> =
            tree.bundles.values().filter(|b| !b.pending).collect();
        if bundles.is_empty() {
            continue;
        }
        bundles.sort_by_key(|b| {
            b.bundle_id
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
        for b in bundles {
            out.push_str(&format!("\n### {} · {}\n\n", b.bundle_id.0, b.title.trim()));
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
/// 真的写入。全部树都无实捆（影子期尾巴会话）= 跳过。
pub fn sync_workspace_tree_md(
    files: &MemoryFiles,
    store: &TreeStore,
    sessions_dir: &Path,
    workspace: &str,
) -> Result<bool, String> {
    let rows = store.scan_workspace(workspace);
    let has_real_bundle = rows
        .iter()
        .any(|(t, _, _)| t.bundles.values().any(|b| !b.pending));
    if !has_real_bundle {
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

/// 首启导出（设计 §6 F1）：存量 sqlite 事实 → md（**文件已存在 = 跳过**，
/// 人的编辑永不被覆盖）+ 各工作区 tree.md（有人改保护）。幂等，每次启动
/// 可跑；返回 (新写事实数, 新写 tree.md 数)。
pub fn run_initial_export(
    memory: &MemoryStore,
    tree_store: &TreeStore,
    files: &MemoryFiles,
    sessions_dir: &Path,
) -> Result<(usize, usize), String> {
    let mut facts = 0usize;
    for r in memory.export_rows()? {
        if files
            .root()
            .join(memlayout::fact_rel_path(&r.workspace, &r.key))
            .exists()
        {
            continue;
        }
        match files.write_fact(
            &r.workspace,
            &r.key,
            &r.value,
            &r.tags,
            &r.source,
            r.updated_at,
        ) {
            Ok(_) => facts += 1,
            Err(e) => tracing::warn!("导出记忆 `{}` 失败: {e}", r.key),
        }
    }
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
    Ok((facts, trees))
}

/// 首启导出入口（state 组装点调用）：后台线程 fire-and-forget；总开关
/// 关 = 什么都不做（回纯 sqlite）。
pub fn spawn_initial_export(
    memory: std::sync::Arc<MemoryStore>,
    tree_store: TreeStore,
    files: MemoryFiles,
    sessions_dir: PathBuf,
    files_on: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
) {
    std::thread::spawn(move || {
        if !files_on() {
            return;
        }
        match run_initial_export(&memory, &tree_store, &files, &sessions_dir) {
            Ok((0, 0)) => {}
            Ok((f, t)) => tracing::info!(facts = f, trees = t, "记忆文件化首启导出完成"),
            Err(e) => tracing::warn!("记忆文件化首启导出失败: {e}"),
        }
    });
}

#[cfg(test)]
mod tests;
