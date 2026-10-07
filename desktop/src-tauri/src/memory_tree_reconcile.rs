//! tree.md 人改对账（M5 编辑面的文件侧，《Memory File-ization Design》§3
//! 同步纪律的树侧落地；F2 记档挂账「标题/摘要回写投影」随 M5 编辑面兑现）。
//!
//! 人直接改 `tree.md` 的捆标题/摘要 = 编辑面另一半：文件新于全部投影
//! （F1 人改保护的同一判据）= 人在整理之后动过文件 → 解析小节，按 range
//! 隐性指针回配投影捆，把人改的标题/摘要写回并打「手动」标记（树 v2
//! §14 纪律 5，自动层此后永不覆盖）。
//!
//! 边界（宁缺勿假）：
//! * 只吸纳**标题/摘要文本**——结构改动（人删小节/改章节号）忽略不删捆，
//!   删除捆走记忆页显式手势（删错无原文损失也要用户知情）；
//! * 文件不新于投影 = 机器写的，跳过（零噪声）；
//! * 投影缺/坏/占位章节 = 该小节跳过（投影可重建，不反推结构）。

use std::path::Path;

use serde::Serialize;

use plugin_api::memlayout;

use memory_tree::TreeStore;

use crate::memory_files::MemoryFiles;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeReconcileSummary {
    /// 扫过的 tree.md 数。
    pub scanned: u32,
    /// 回写的标题数。
    pub titles: u32,
    /// 回写的摘要数。
    pub summaries: u32,
    pub warnings: u32,
}

impl TreeReconcileSummary {
    pub fn has_action(&self) -> bool {
        self.titles + self.summaries > 0
    }
}

/// 一条从 md 解析出的小节（title/summary + 隐性指针）。
struct ParsedSection {
    title: String,
    summary: String,
    range: (usize, usize),
    session: String,
}

/// 跑一轮 tree.md 对账。`known_workspaces` = fsRoots ∪ sqlite 键位（目录
/// 名反查归属，与事实 reconcile 同款）。总开关由调用方裁决。
pub fn run(
    tree_store: &TreeStore,
    files: &MemoryFiles,
    known_workspaces: &[String],
) -> TreeReconcileSummary {
    let mut sum = TreeReconcileSummary::default();
    let root = files.root();
    let projects = root.join("projects");
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return sum; // 层缺席 = 没有可对账的文件
    };
    let by_dir: std::collections::HashMap<String, String> = known_workspaces
        .iter()
        .map(|ws| (memlayout::project_dir_name(ws), ws.clone()))
        .collect();
    for e in entries.flatten() {
        let dir_name = e.file_name().to_string_lossy().into_owned();
        let md = e.path().join("tree.md");
        if !md.is_file() {
            continue;
        }
        let workspace = if dir_name == "_unaffiliated" {
            String::new()
        } else {
            match by_dir.get(&dir_name) {
                Some(ws) => ws.clone(),
                None => {
                    sum.warnings += 1;
                    tracing::warn!("tree.md 对账跳过 {dir_name}: 目录无法反查项目");
                    continue;
                }
            }
        };
        if let Err(err) = reconcile_one(tree_store, &md, &workspace, &mut sum) {
            sum.warnings += 1;
            tracing::warn!("tree.md 对账失败（{workspace}）: {err}");
        }
    }
    sum
}

fn reconcile_one(
    tree_store: &TreeStore,
    md: &Path,
    workspace: &str,
    sum: &mut TreeReconcileSummary,
) -> Result<(), String> {
    sum.scanned += 1;
    // 人改判据与 F1 写保护同源：文件 mtime 新于全部投影才值得解析
    let rows = tree_store.scan_workspace(workspace);
    let newest = rows.iter().map(|(_, _, ms)| *ms).max().unwrap_or(0);
    let md_ms = memlayout::mtime_ms(md).unwrap_or(0);
    if newest == 0 || md_ms <= newest {
        return Ok(()); // 机器写的 / 投影缺席——零噪声跳过
    }
    let text = std::fs::read_to_string(md).map_err(|e| format!("读取失败: {e}"))?;
    let sections = parse_sections(&text);
    if sections.is_empty() {
        return Ok(());
    }
    // 按 session 分组回配（一个 md 覆盖多个会话小节）
    let mut by_session: std::collections::HashMap<&str, Vec<&ParsedSection>> =
        std::collections::HashMap::new();
    for s in &sections {
        by_session.entry(s.session.as_str()).or_default().push(s);
    }
    for (session_id, list) in by_session {
        let Some(tree) = tree_store.load(workspace, session_id) else {
            continue; // 投影缺 = 结构不可反推（重建兜底）
        };
        let mut tree = tree;
        let mut dirty = false;
        for s in list {
            let Some(b) = tree
                .chapters
                .values_mut()
                .find(|b| b.event_range == s.range && !b.pending)
            else {
                continue; // 区间对不上/占位：忽略（人删投影或结构改动）
            };
            if b.title != s.title {
                b.title = s.title.clone();
                b.title_edited_by_user = true;
                sum.titles += 1;
                dirty = true;
            }
            if b.summary.trim() != s.summary {
                b.summary = s.summary.clone();
                b.summary_edited_by_user = true;
                sum.summaries += 1;
                dirty = true;
            }
        }
        if dirty {
            tree_store
                .save(&tree)
                .map_err(|e| format!("投影写回失败: {e}"))?;
            tracing::info!(
                workspace = %workspace, session = %session_id,
                titles = sum.titles, summaries = sum.summaries,
                "tree.md 人改已回写投影（打手动标记）"
            );
        }
    }
    Ok(())
}

/// 解析 tree.md 小节（渲染格式 memory_files::render_tree_md 的宽松读回，
/// 与 parse_fact 同哲学——人写的文件不严格）。
fn parse_sections(text: &str) -> Vec<ParsedSection> {
    let mut out: Vec<ParsedSection> = Vec::new();
    let mut cur: Option<(String, Vec<String>)> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            // 新小节头：`### b3 · 标题`
            let title = rest.split_once('·').map(|(_, t)| t.trim()).unwrap_or("");
            cur = Some((title.to_string(), Vec::new()));
            continue;
        }
        if trimmed.starts_with("<!-- range:") {
            if let Some((title, summary_lines)) = cur.take() {
                if let Some(range) = parse_range(trimmed) {
                    let session = parse_session(trimmed);
                    out.push(ParsedSection {
                        title,
                        summary: join_summary(summary_lines),
                        range,
                        session,
                    });
                }
            }
            continue;
        }
        if trimmed.starts_with("## ") || trimmed.starts_with("# ") {
            cur = None; // 会话/文档标题边界：悬挂小节（无指针）丢弃
            continue;
        }
        if let Some((_, summary_lines)) = cur.as_mut() {
            if !trimmed.is_empty() && !trimmed.starts_with('>') {
                summary_lines.push(trimmed.to_string());
            }
        }
    }
    out
}

/// `<!-- range: 10-19 · ... -->` → (10, 19)；解析失败 = None（跳过）。
fn parse_range(comment: &str) -> Option<(usize, usize)> {
    let rest = comment.strip_prefix("<!-- range:")?;
    let end = rest.find('·').unwrap_or(rest.find("-->")?);
    let pair = rest[..end].trim();
    let (a, b) = pair.split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// `... session: abc -->` → `abc`；缺 = 空串（跳过该小节）。
fn parse_session(comment: &str) -> String {
    comment
        .split("session:")
        .nth(1)
        .map(|s| s.trim().trim_end_matches("-->").trim().to_string())
        .unwrap_or_default()
}

fn join_summary(lines: Vec<String>) -> String {
    lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests;
