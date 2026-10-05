//! 记忆文件 reconcile（F2，《Memory File-ization Design》§3/§7）：人的
//! 编辑面落地——md 是权威编辑面，sqlite 是机器索引（衍生，可重建）。
//! diff 文件 ↔ 索引，**以文件为准回写索引**；触发 = 启动 + 记忆页打开
//! + 手动命令。
//!
//! 判定矩阵（per 文件，标记 = 本进程族上次落笔的 mtime 快照）：
//! * 正文 == 索引值（尾空白不敏感）→ 已同步：补标记即走（不产生更新噪声，
//!   人格式化过的文件不被改写）
//! * 正文 ≠ 索引值 且 标记 == mtime → 文件自落笔未动、索引先行（历史
//!   同步失败）→ 从索引**愈合**文件
//! * 正文 ≠ 索引值 且（无标记 或 mtime ≠ 标记）→ 文件被外部改过（或从未
//!   纳入管理）→ **文件为准**：更新 / 新增索引行
//! * 行在而文件缺 且 标记在 → 人删了该文件（单文件手势）→ 删索引行
//! * scope 目录整体缺席 / memory 根缺席 = 复位手势 → 不删行（首启导出按
//!   索引重建文件）
//!
//! 索引写走 `MemoryStore::reconcile_*` 专用路径（绝不反向同步 md）；备份
//! 文件（`· 人工编辑备份`，写路径冲突保护产物）按 frontmatter name 当普通
//! 新事实吸纳。tree.md 不在本批范围（人改保护 F1 已挡覆盖；标题/摘要回写
//! 投影随 M5 编辑面——避免两处编辑回写实现）。

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::Serialize;

use plugin_api::memlayout::{self, ParsedFact};

use crate::memory::{MemoryRow, MemoryStore};
use crate::memory_files::MemoryFiles;

/// 与 memory.rs 同限（跨模块可见性从这里引用，避免第二份常量）。
use crate::memory::{KEY_MAX, VALUE_MAX};

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileSummary {
    pub scanned: u32,
    pub created: u32,
    pub updated: u32,
    pub healed: u32,
    pub deleted: u32,
    pub warnings: u32,
}

impl ReconcileSummary {
    /// 是否有任何实际动作（启动日志只在有动作时打印）。
    pub fn has_action(&self) -> bool {
        self.created + self.updated + self.healed + self.deleted > 0
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
    if let Ok(entries) = std::fs::read_dir(root.join("projects")) {
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
        let facts_dir = scope_facts_dir(root, ws);
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

/// 键位的 facts 目录（删除判定用）：全局 = `global/`，项目 = `projects/<dir>/facts/`。
fn scope_facts_dir(root: &Path, workspace: &str) -> PathBuf {
    if workspace.is_empty() {
        root.join("global")
    } else {
        root.join("projects")
            .join(memlayout::project_dir_name(workspace))
            .join("facts")
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
        Some(r) if marker == Some(actual) => {
            // 文件未动、索引先行（历史 md 同步失败）→ 从索引愈合文件
            files.write_fact(&workspace, &key, &r.value, &r.tags, &r.source, r.updated_at)?;
            sum.healed += 1;
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

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
