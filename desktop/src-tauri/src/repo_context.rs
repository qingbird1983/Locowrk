//! 仓库交接层（F2，《Memory File-ization Design》§2.2；v2.136 按新记忆
//! 模型改写）：`<repo>/.Locowrk/` 引导 + `<repo_context>` **静态提醒段**。
//!
//! v2.136 裁定「项目侧全提醒制」：项目记忆（requirements / handoff /
//! notes）**正文不再注入流水线**——由当前工作的主模型自己在工作中按需
//! 读/写/改（软件不代劳，记忆管理器只管上下文打包整理）；本模块只在
//! 注入段给出节点提醒钩子（开场先读、状态实质变化时更新），文本静态 =
//! 跨轮逐字节稳定（M1 预取纪律不破，也退出 scope_id 缓存键）。
//!
//! 写边界（宁严勿松）不变：替用户做的仓库改动 = 建 `.Locowrk/` +
//! requirements.md 模板 + `.gitignore` 追加一行，全部幂等；handoff.md 与
//! notes/ 由主模型经文件工具维护，本模块只读。

use std::path::Path;

/// 交接目录名（仓库根下）。
pub const DIR: &str = ".Locowrk";
const REQUIREMENTS: &str = "requirements.md";

/// requirements.md 模板（首次引导写入；用户与模型共同维护，HTML 注释
/// 自说明。v2.136：注入改提醒制后不再有"只读注入"语义）。
const REQUIREMENTS_TEMPLATE: &str = "# 项目要求（requirements）\n\n<!-- \
本文件由 Locowrk 维护骨架：项目简介 / 要求 / 注意点，用户与模型共同维护。\
模型开工前先读这里；有新的长期约定也写回本文件。\n\
整个 .Locowrk/ 目录已加入 .gitignore，不入库。 -->\n\n\
- \n";

/// 引导（幂等）：`.Locowrk/` 缺则建；requirements.md 缺则写模板；是 git
/// 仓库（`.git` 在）且 `.gitignore` 无该行 → 追加 `.Locowrk/`。
pub fn ensure_layout(repo: &Path) -> Result<(), String> {
    let dir = repo.join(DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("交接目录创建失败 {}: {e}", dir.display()))?;
    let req = dir.join(REQUIREMENTS);
    if !req.exists() {
        std::fs::write(&req, REQUIREMENTS_TEMPLATE)
            .map_err(|e| format!("requirements 模板写入失败 {}: {e}", req.display()))?;
    }
    if repo.join(".git").exists() {
        append_gitignore(repo)?;
    }
    Ok(())
}

/// `.gitignore` 追加一行 `.Locowrk/`（已有该行 = 跳过；无文件 = 创建）。
fn append_gitignore(repo: &Path) -> Result<(), String> {
    let gi = repo.join(".gitignore");
    let entry = format!("{DIR}/");
    let content = std::fs::read_to_string(&gi).unwrap_or_default();
    if content.lines().any(|l| l.trim() == entry) {
        return Ok(());
    }
    let mut next = content;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&entry);
    next.push('\n');
    std::fs::write(&gi, next).map_err(|e| format!(".gitignore 追加失败 {}: {e}", gi.display()))
}

/// `<repo_context>` 提醒段内文（静态文本，不含标签——内核侧统一包裹）。
/// **与文件内容无关**（正文不注入）：列出目录里有什么 + 两个节点钩子。
/// v2.139 按提示词分层设计 §5.1 改写：read 侧从「开场必读」（START +
/// do not wait to be asked）改条件触发（请求触到项目工作才读，每任务至多
/// 一次，不做开场仪式）——治「刚问第一句话就去读项目文件」的开场仪式；
/// write 侧主动性保留（状态实质变化时自行更新）。跨轮逐字节稳定 = 预取
/// 缓存键只需会话键位，文件 mtime 不再参与（memory.rs scope_id）。
pub fn build(_repo: &Path) -> Option<String> {
    Some(format!(
        "Project working memory lives in {DIR}/ of this repo (git-ignored) and is \
         yours to maintain with file tools:\n\
         - requirements.md — project profile, requirements and standing caveats.\n\
         - handoff.md — current work state (done / in progress / next steps) for \
         future sessions.\n\
         - notes/ — work logs and lessons learned (one dated file per topic).\n\
         When the user's request builds on prior project work, consult \
         requirements.md and handoff.md first — at most once per task, and never \
         as an opening ritual when the request doesn't touch them. When the \
         project state materially changes (task finished, direction changed, \
         hard-won lesson learned), update handoff.md yourself and append a dated \
         note. These files are NOT auto-injected — you maintain them."
    ))
}

#[cfg(test)]
mod tests;
