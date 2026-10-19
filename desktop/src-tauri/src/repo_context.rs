//! 仓库交接层（F2，《Memory File-ization Design》§2.2；v2.136 按新记忆
//! 模型改写；**S3 刀② 起注入真实正文**）：`<repo>/.Locowrk/` 引导 +
//! `<repo_context>` 交接段。
//!
//! 形状沿革（这一格改过三次，别再往回漂）：
//! - v2.136「项目侧全提醒制」：正文不注入，只给一段**静态**提醒（目录里有什么、
//!   在什么节点读/更新）。代价是"新会话一眼见现状"这件事从未真发生过——
//!   `build(_repo)` 连参数都不消费，S3 取证读实的就是这一条。
//! - 方案稿 §F（2026-10-17 重定）：交接层改**两文档制**——`MEMORY.md`（现状，按主题
//!   分节、原地替换、硬顶）+ `journal/<YYYY-MM-DD>.md`（流水，按天封卷）。旧的
//!   `handoff.md` / `notes/` 那一套的病根是"把流水写进现状文档"。
//! - **S3 刀②**：`<repo_context>` 的注入源切到 §F 那份 `MEMORY.md` 正文（+ requirements
//!   提醒行），journal 不注入、按需读。正文进 system 就打破了"跨轮逐字节稳定"，
//!   所以同一刀把**文件 mtime 接进预取缓存键**（`MemorySource::index_stamp`）——
//!   人改交接文件 = 下一轮生效，而不是整个会话钉死在旧份上。
//!
//! 写边界（宁严勿松）不变：替用户做的仓库改动 = 建 `.Locowrk/` + requirements.md 模板 +
//! MEMORY.md 骨架 + `.gitignore` 追加一行，全部幂等、已存在一律不碰；正文由主模型
//! 经文件工具维护，本模块只读。

use std::path::Path;

/// 交接目录名（仓库根下）。
pub const DIR: &str = ".Locowrk";
const REQUIREMENTS: &str = "requirements.md";
/// 交接主入口（方案稿 §F 两文档制里"现状"那一份）。**旧名 `handoff.md` 已废**：
/// 那份单文件把"流水"写进"现状文档"，只追加不替换、没有尺寸顶 ⇒ 实测无限膨胀
/// （§F 的病因诊断）。同名 `MEMORY.md` 与记忆两层的那个是**同一个约定的不同文件**
/// （这里坐在 `<repo>/.Locowrk/` 下，那两个坐在数据根下），别拿错路径。
/// `pub(crate)` 的理由 = 预取指纹（`memory::source::index_stamp`）要 stat 同一个文件，
/// 在那边再写一遍 `"MEMORY.md"` 就是第二个出处。
pub(crate) const HANDOFF: &str = "MEMORY.md";

/// `MEMORY.md` 骨架（首次引导写入；§F 的主题分节）。按**主题**分节而不是按时间，
/// 三条都是空壳 ⇒ 模型第一次读到就知道往哪一节写、该替换哪一节。
const HANDOFF_TEMPLATE: &str = "# 现状（交接主入口）\n\n\
## 当前阶段\n- \n\n## 各主线\n- \n\n## 环境与坑\n- \n";

/// requirements.md 模板（首次引导写入；用户与模型共同维护，HTML 注释
/// 自说明。v2.136：注入改提醒制后不再有"只读注入"语义）。
const REQUIREMENTS_TEMPLATE: &str = "# 项目要求（requirements）\n\n<!-- \
本文件由 Locowrk 维护骨架：项目简介 / 要求 / 注意点，用户与模型共同维护。\
模型开工前先读这里；有新的长期约定也写回本文件。\n\
整个 .Locowrk/ 目录已加入 .gitignore，不入库。 -->\n\n\
- \n";

/// 引导（幂等）：`.Locowrk/` 缺则建；requirements.md 与 MEMORY.md 缺则各写模板；
/// 是 git 仓库（`.git` 在）且 `.gitignore` 无该行 → 追加 `.Locowrk/`。
///
/// S3 刀② 起 `MEMORY.md` 也在这里引导：**没有它就没有交接层注入**（`build()` 读不到
/// 就整段缺席），所以"注入源切到真实文件"这件事必须配一个骨架，否则第一个会话什么也
/// 收不到。两份模板都是"缺则写"，已存在就一个字都不碰（人写的那份是权威）。
pub fn ensure_layout(repo: &Path) -> Result<(), String> {
    let dir = repo.join(DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("交接目录创建失败 {}: {e}", dir.display()))?;
    let req = dir.join(REQUIREMENTS);
    if !req.exists() {
        std::fs::write(&req, REQUIREMENTS_TEMPLATE)
            .map_err(|e| format!("requirements 模板写入失败 {}: {e}", req.display()))?;
    }
    let handoff = dir.join(HANDOFF);
    if !handoff.exists() {
        std::fs::write(&handoff, HANDOFF_TEMPLATE)
            .map_err(|e| format!("交接 MEMORY.md 骨架写入失败 {}: {e}", handoff.display()))?;
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

/// `<repo_context>` 提醒段内文（S3 刀② 起 = **交接主入口正文 + 三行提醒**）。
///
/// 方案稿 §F 的原话是"注入 = `<repo_context>` 携带 `MEMORY.md` 正文（它就是为
/// "新会话一眼见现状"而生的交接主入口）+ requirements 提醒行；journal 不注入、按需读"。
/// 刀② 之前这一格是**纯静态串**（`build(_repo)` 连参数都不消费），所以"新会话一眼见
/// 现状"这件事从来没有真发生过。
///
/// **读不到 `MEMORY.md` ⇒ 整段 `None`**（不是退回那份静态提醒）：留一份与文件无关的
/// 提醒会把"这一层没接线"与"接了但暂时没内容"混成同一个形状，而 §E 那枚开关要的恰恰
/// 是前者可辨。生产路径上 `repo_context()` 会先 `ensure_layout` 引导，所以 `None`
/// 实际只出现在未归属 / 开关关 / 目录读不动三种场合。
///
/// 正文按 §F 的硬顶钳（[`HANDOFF_MAX_LINES`] 行 / [`HANDOFF_MAX_BYTES`] 字节）：写侧
/// 钳今天**还不存在**（S1 只钳了记忆库两层的 `MEMORY.md`），所以预算必须站在读侧，
/// 否则人把交接文件写到 500 行就是每轮白烧一次上下文。这条差异登记在下面「挂账」。
pub fn build(repo: &Path) -> Option<String> {
    let path = repo.join(DIR).join(HANDOFF);
    let body = std::fs::read_to_string(&path).ok()?;
    let (kept, truncated) = clamp_body(&body);
    let mut out = format!(
        "Project working memory (current state) — {DIR}/{HANDOFF}, injected verbatim below. \
         It is the handoff entry point: read it instead of re-deriving the project's state \
         from chat.\n\
         {kept}\n\
         Maintenance: when the project state materially changes (task finished, direction \
         changed, hard-won lesson learned), update this file yourself — replace the stale \
         section in place, never append a second copy of \"current state\". Dated logs go \
         into {DIR}/journal/<YYYY-MM-DD>.md (yesterday's file is sealed); those are NOT \
         injected, read them on demand. {DIR}/{REQUIREMENTS} holds the project's standing \
         requirements — consult it when the request builds on them, at most once per task \
         and never as an opening ritual."
    );
    if truncated {
        out.push_str(&format!("\n\n{TRUNCATED_NOTE}"));
    }
    Some(out)
}

/// 交接正文的硬顶（§F「200 行 / 10KB」）。**与 `memlayout::MAX_INDEX_LINES` 同值是裁定
/// 而不是同一件事**：那一颗管的是记忆索引指针行的预算，这一颗管的是交接正文，
/// 复用会让"调索引预算"顺手改掉交接预算 ⇒ 各留一处，注释互相指认。
pub const HANDOFF_MAX_LINES: usize = 200;
pub const HANDOFF_MAX_BYTES: usize = 10 * 1024;

/// 超限自描述（不另造文案：截断必须让模型知道"看到的是半份"，否则它以为现状就这些）。
const TRUNCATED_NOTE: &str =
    "（MEMORY.md truncated at the handoff budget — open the file for the full text.）";

/// 行 + 字节双钳：任一超限即截断并回报（与 `memory_index::clamp` 同形，但**不共用实现**
/// —— 那一把钳的是指针行列表并附索引专属的 WARNING，两件事凑一个函数就是下一刀的分叉）。
fn clamp_body(body: &str) -> (String, bool) {
    let mut kept: Vec<&str> = Vec::new();
    let mut bytes = 0usize;
    let mut truncated = false;
    for (i, line) in body.lines().enumerate() {
        if i >= HANDOFF_MAX_LINES {
            truncated = true;
            break;
        }
        // +1 = 这一行前面那个换行（首行除外，它前面没有）。
        bytes += line.chars().count() + usize::from(!kept.is_empty());
        if bytes > HANDOFF_MAX_BYTES {
            // 超预算那一行**整个吐掉**而不是给半行：半行会被模型读成一条完整事实。
            truncated = true;
            break;
        }
        kept.push(line);
    }
    (kept.join("\n"), truncated)
}

#[cfg(test)]
mod tests;
