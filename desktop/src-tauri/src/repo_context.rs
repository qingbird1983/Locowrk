//! 仓库交接层（F2，《Memory File-ization Design》§2.2）：`<repo>/.Locowrk/`
//! 引导 + `<repo_context>` 注入段内容。写边界（宁严勿松）：替用户做的仓库
//! 改动 = 建 `.Locowrk/` + requirements.md 模板 + `.gitignore` 追加一行
//! （用户方向①原话「主动要求加入 git 排除」），全部幂等；handoff.md 与
//! notes/ 由 F3 的收束更新与模型工具写入，本模块只读。
//!
//! M1 预取纪律兼容（设计 §2.2）：注入段随文件 mtime 进 `scope_id` 缓存键
//! （[`fingerprint`]）——人改文件 = 显式变更自动重算，键不变逐字节复用。

use std::path::Path;

/// 交接目录名（仓库根下）。
pub const DIR: &str = ".Locowrk";
const REQUIREMENTS: &str = "requirements.md";
pub const HANDOFF: &str = "handoff.md";
const NOTES: &str = "notes";
/// 注入段单文件上限（字符）——超长截断标注，全文在仓库里人可看可改。
const FILE_CHARS: usize = 2000;

/// requirements.md 模板（首次引导写入；用户为主编辑，HTML 注释自说明）。
const REQUIREMENTS_TEMPLATE: &str = "# 项目要求（requirements）\n\n<!-- \
本文件由 Locowrk 维护：项目要求，用户为主编辑，模型只读注入上下文。\n\
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

/// 交接文件组 mtime 指纹（scope_id 缓存键成分）：requirements / handoff
/// 的 mtime + notes/ 清单（文件名 + mtime，增删改都变键）。无目录 = 空串
/// （不参与键）。
pub fn fingerprint(repo: &Path) -> String {
    let dir = repo.join(DIR);
    if !dir.is_dir() {
        return String::new();
    }
    let mut fp = String::new();
    for name in [REQUIREMENTS, HANDOFF] {
        fp.push_str(&format!("{name}:{};", mtime_of(&dir.join(name))));
    }
    if let Ok(entries) = std::fs::read_dir(dir.join(NOTES)) {
        for e in entries.flatten() {
            fp.push_str(&format!(
                "n:{}:{};",
                e.file_name().to_string_lossy(),
                mtime_of(&e.path())
            ));
        }
    }
    fp
}

fn mtime_of(p: &Path) -> u128 {
    std::fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 注入段内文（不含 `<repo_context>` 标签——内核侧统一包裹）。None =
/// 无目录（未归属 / 引导缺席）。有目录但内容全空 → 一行提示段（交接面
/// 已就位，人与模型都知道能放什么）；有内容 → requirements 全文（截断）
/// + handoff 全文（截断）+ notes/ 文件名清单（按需用文件工具读全文）。
pub fn build(repo: &Path) -> Option<String> {
    let dir = repo.join(DIR);
    if !dir.is_dir() {
        return None;
    }
    let req = read_clip(&dir.join(REQUIREMENTS));
    let handoff = read_clip(&dir.join(HANDOFF));
    let notes = list_notes(&dir);
    if req.is_none() && handoff.is_none() && notes.is_empty() {
        return Some(format!(
            "Project handoff directory is ready at {DIR}/ (git-ignored): \
             requirements.md awaits your edits; handoff.md and notes/ may be \
             maintained there as work progresses."
        ));
    }
    let mut s = String::from(
        "Files below live in the project's .Locowrk/ directory (git-ignored). \
         requirements.md is authored by the user (read-only for you); \
         handoff.md and notes/ record work progress and may be maintained.\n",
    );
    if let Some(r) = req {
        s.push_str(&format!("\n--- requirements.md ---\n{r}\n"));
    }
    if let Some(h) = handoff {
        s.push_str(&format!("\n--- handoff.md ---\n{h}\n"));
    }
    if !notes.is_empty() {
        s.push_str("\n--- notes/ ---\n");
        for n in notes {
            s.push_str(&format!("- {n}\n"));
        }
        s.push_str("(read a note with file tools when needed)\n");
    }
    Some(s)
}

/// 读文件并截断（缺 / 空文件 = None——空内容不占注入段）。
fn read_clip(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    let text = text.trim_end().to_string();
    if text.is_empty() {
        return None;
    }
    let count = text.chars().count();
    if count > FILE_CHARS {
        Some(format!(
            "{} …(truncated)",
            text.chars().take(FILE_CHARS).collect::<String>()
        ))
    } else {
        Some(text)
    }
}

fn list_notes(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join(NOTES))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".md"))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests;
