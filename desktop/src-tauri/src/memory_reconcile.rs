//! 记忆文件对账（《Memory File-ization Design v1.1》§8 决策点 1 之后**只剩一个方向**）：
//! **真源 = md 文件**，sqlite 是刀③ 之前插件那六枚工具仍在读的**影子**。本模块做的
//! 是"文件为准，把影子对齐过去"；触发 = 启动（后台）+ 记忆页挂载 + 开发者手动按钮。
//!
//! ⚠ S1 刀②c-2 起**「影子 → 文件」这个方向的所有写动作全部停止**（旧形状里有两条：
//! 首启导出按索引补文件、`healed` 那一支"文件未动而索引先行 ⇒ 从索引愈合文件"）。
//! 它们在真源 = 镜像的时代是对的，在真源 = 文件之后含义正好反过来：**把用户已经删掉
//! 的记忆在下次启动时写回来** ⇒ AC-S1-6 那三条遗忘验证不成立。这两条留下的痕迹：
//! 遇到"影子比真源新而文件没动过"这种状态，现在只记 warning 并落日志，**一个字节都不写文件**。
//!
//! 判定矩阵（per 文件，标记 = 本进程族上次落笔的 mtime 快照）：
//! * 正文 == 索引值（尾空白不敏感）→ 已同步：补标记即走（不产生更新噪声，
//!   人格式化过的文件不被改写）
//! * 正文 ≠ 索引值 且 标记 == mtime → 影子比真源新 →
//!   **不动文件**，记 warning（成因原本是"插件写库而文件没跟上"，S1 刀③b 之后插件
//!   不再开这张表 ⇒ 这一格只剩**跨版本留下的旧影子**；宿主 `shadow_*` 也退役之后，
//!   它连历史残留都会被下一次对账消化掉）
//! * 正文 ≠ 索引值 且（无标记 或 mtime ≠ 标记）→ 文件被外部改过（或从未
//!   纳入管理）→ **文件为准**：更新 / 新增影子行
//! * 行在而文件缺 且 标记在 → 人删了该文件（单文件手势）→ 删影子行
//! * scope 目录整体缺席 / memory 根缺席 = 复位手势 → 不删行（**也不再补文件**：
//!   真源里没有的东西，不该由一份副本重新造出来）
//!
//! 影子写走 `MemoryStore::reconcile_*` 专用路径（绝不反向同步 md）；备份文件
//! （`· 人工编辑备份`，写路径冲突保护产物）按 frontmatter name 当普通新事实吸纳。
//! tree.md 不在本件范围（那是投影不是真源，人改保护与重建另走
//! `memory_tree_reconcile` 与 `memory_files`）。

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::Serialize;

use plugin_api::memlayout::{self, ParsedFact};

use crate::memory::{MemoryRow, MemoryStore};
use crate::memory_files::MemoryFiles;

/// 与 memory.rs 同限（跨模块可见性从这里引用，避免第二份常量）。
use crate::memory::{KEY_MAX, VALUE_MAX};
use crate::timeutil::now_secs;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileSummary {
    pub scanned: u32,
    pub created: u32,
    pub updated: u32,
    pub deleted: u32,
    /// 两种"看见了但没动"都记在这里：单文件处理失败 + 影子比真源新（②c-2 起不写文件）。
    /// 后者在刀③ 摘掉影子写之后不可能再出现。
    pub warnings: u32,
}

impl ReconcileSummary {
    /// 是否有**对齐动作**（启动日志只在有动作时打印）。
    /// `warnings` 不算动作——它恰恰是"什么都没改"那一档。
    pub fn has_action(&self) -> bool {
        self.created + self.updated + self.deleted > 0
    }
}

/// 跑一轮 reconcile。`known_workspaces` = fsRoots ∪ sqlite 全部键位——
/// 人新建的无 frontmatter 文件按项目目录名反查归属。
pub fn run(
    memory: &MemoryStore,
    files: &MemoryFiles,
    known_workspaces: &[String],
) -> Result<ReconcileSummary, String> {
    let mut sum = ReconcileSummary::default();
    let root = files.root();
    if !root.exists() {
        return Ok(sum); // 整层缺席 = 复位手势：索引不动，导出按索引重建
    }
    // 目录名 → workspace 反查表（人建文件没写 frontmatter workspace 时用）
    let by_dir: HashMap<String, String> = known_workspaces
        .iter()
        .map(|ws| (memlayout::project_dir_name(ws), ws.clone()))
        .collect();
    // 全量行索引（一次拉取，避免逐文件开连接）
    let rows: HashMap<(String, String), MemoryRow> = memory
        .export_rows()?
        .into_iter()
        .map(|r| ((r.workspace.clone(), r.key.clone()), r))
        .collect();

    // ---- 文件优先：global/ + projects/*/facts/ 下每个 .md ----
    let mut files_to_check: Vec<(PathBuf, Scope)> = Vec::new();
    let global = root.join("global");
    if global.is_dir() {
        collect_md(&global, &mut files_to_check, Scope::Global);
    }
    if let Ok(entries) = std::fs::read_dir(memlayout::projects_dir(root)) {
        for e in entries.flatten() {
            let facts = e.path().join("facts");
            if facts.is_dir() {
                collect_md(&facts, &mut files_to_check, Scope::ProjectDir);
            }
        }
    }
    for (path, scope) in files_to_check {
        if let Err(e) = process_file(memory, files, &rows, &by_dir, &path, scope, &mut sum) {
            sum.warnings += 1;
            tracing::warn!("reconcile 跳过 {}: {e}", path.display());
        }
    }

    // ---- 行优先：行在文件缺（标记在 = 我们曾写过）→ 人删文件 → 删行 ----
    let mut scopes: BTreeSet<String> = memory.all_workspaces()?.into_iter().collect();
    scopes.extend(known_workspaces.iter().cloned());
    let mut dirty_scopes: Vec<String> = Vec::new();
    for ws in &scopes {
        // 路径一律经 memlayout（Q15 收口：这里以前自己拼第二份 `projects/<dir>/facts`）。
        // Err 支在现状不可达（`project_dir` 只对 `''` 报错，而 `''` 上面已分流）——
        // 保留它而不是 unwrap，是为了让"哪天算式改了"表现为**跳过 + warning**，
        // 而不是拿着一个错目录去删行。
        let facts_dir = match scope_facts_dir(root, ws) {
            Ok(p) => p,
            Err(e) => {
                sum.warnings += 1;
                tracing::warn!("reconcile 跳过键位（{ws}）: {e}");
                continue;
            }
        };
        if !facts_dir.is_dir() {
            continue; // 目录整体缺席 = 复位手势，不删行
        }
        for r in memory.reconcile_rows(ws)? {
            let rel = memlayout::fact_rel_path(ws, &r.key);
            if !root.join(&rel).exists() && memlayout::read_marker(root, &rel).is_some() {
                memory.reconcile_delete(ws, &r.key)?;
                sum.deleted += 1;
                dirty_scopes.push(ws.clone());
            }
        }
    }
    for ws in &dirty_scopes {
        if let Err(e) = memlayout::refresh_index(root, ws) {
            sum.warnings += 1;
            tracing::warn!("reconcile 索引刷新失败（{ws}）: {e}");
        }
    }
    Ok(sum)
}

#[derive(Clone, Copy)]
enum Scope {
    /// `global/` = 未归属键位 `''`。
    Global,
    /// `projects/<dir>/facts/`——workspace 按文件 frontmatter / 目录反查。
    ProjectDir,
}

fn collect_md(dir: &Path, out: &mut Vec<(PathBuf, Scope)>, scope: Scope) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".md") && name != "MEMORY.md" {
                out.push((e.path(), scope));
            }
        }
    }
}

/// 键位的 facts 目录（删除判定用）：全局 = `global/`，项目 = `projects/<dir>/facts/`
/// ——项目支不自拼，转 `memlayout::project_dir`（同一条路径只许一处算法）。
fn scope_facts_dir(root: &Path, workspace: &str) -> Result<PathBuf, String> {
    if workspace.is_empty() {
        Ok(root.join("global"))
    } else {
        Ok(memlayout::project_dir(root, workspace)?.join("facts"))
    }
}

fn process_file(
    memory: &MemoryStore,
    files: &MemoryFiles,
    rows: &HashMap<(String, String), MemoryRow>,
    by_dir: &HashMap<String, String>,
    path: &Path,
    scope: Scope,
    sum: &mut ReconcileSummary,
) -> Result<(), String> {
    sum.scanned += 1;
    let text = std::fs::read_to_string(path).map_err(|e| format!("读取失败: {e}"))?;
    let parsed: ParsedFact = memlayout::parse_fact(&text);
    // 身份：frontmatter name 权威，缺 = 文件名主干（人新建常见）
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let key = parsed.name.clone().unwrap_or(stem);
    // 键位：global 固定 ''；项目 = frontmatter workspace 原值 → 目录反查
    let workspace = match scope {
        Scope::Global => String::new(),
        Scope::ProjectDir => match parsed.workspace.clone() {
            Some(ws) if !ws.is_empty() => ws,
            _ => {
                let dir = path
                    .ancestors()
                    .nth(2)
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                by_dir
                    .get(&dir)
                    .cloned()
                    .ok_or_else(|| format!("目录 {dir} 无法反查项目（无 frontmatter workspace）"))?
            }
        },
    };
    if key.is_empty() || key.len() > KEY_MAX {
        return Err(format!("key 长度越界（1..={KEY_MAX}）"));
    }
    let rel = memlayout::fact_rel_path(&workspace, &key);
    let marker = memlayout::read_marker(files.root(), &rel);
    let actual = memlayout::mtime_ms(path).ok_or("文件竞态消失")?;
    match rows.get(&(workspace.clone(), key.clone())) {
        Some(r) if body_eq(&parsed.body, &r.value) => {
            // 已同步：补标记（人重存过 / 首次纳入管理都不产生更新噪声）
            let _ = memlayout::write_marker(files.root(), &rel, actual);
        }
        Some(_) if marker == Some(actual) => {
            // 影子比真源新而文件没动过（sqlite 落笔成功、md 侧没跟上）。旧形状在这里
            // 是"从索引愈合文件"，真源 = 文件之后**不许再写**：文件里没这句话，而索引
            // 只是副本，副本单方面把真源改掉 = 人删掉的记忆下次启动复活（AC-S1-6）。
            // 只点名、不写盘，刀③ 摘掉影子写之后这一格不可能再出现。
            sum.warnings += 1;
            tracing::warn!(
                "reconcile：影子比真源新，未写文件 ws={workspace:?} key={key:?}（刀③ 摘影子后不可达）"
            );
        }
        Some(r) => {
            // 文件被外部改过 → 文件为准更新行（tags/source 沿行；updated = 现在）
            memory.reconcile_upsert(
                &workspace,
                &key,
                &parsed.body,
                &r.tags,
                &r.source,
                now_secs(),
            )?;
            let _ = memlayout::write_marker(files.root(), &rel, actual);
            sum.updated += 1;
        }
        None => {
            // 索引没有 → 人建的新事实（或行被删文件还在）：文件为准新增
            if parsed.body.len() > VALUE_MAX {
                return Err(format!("value 超过 {VALUE_MAX} 字节上限"));
            }
            let at = parsed.updated_at.unwrap_or(actual / 1000);
            let source = parsed.source.as_deref().unwrap_or("human");
            memory.reconcile_upsert(&workspace, &key, &parsed.body, "", source, at)?;
            let _ = memlayout::write_marker(files.root(), &rel, actual);
            sum.created += 1;
        }
    }
    Ok(())
}

/// 正文等价（尾空白不敏感）：人重存 / 首次纳管不当作编辑。
fn body_eq(a: &str, b: &str) -> bool {
    a.trim_end() == b.trim_end()
}

#[cfg(test)]
mod tests;
