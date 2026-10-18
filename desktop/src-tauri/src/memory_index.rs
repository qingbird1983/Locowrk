//! 项目记忆**索引**注入（MR 稿 §4.1，队列 Q2）。
//!
//! **这块治什么**：`<persistent_memory>` 只注全局层，而全局层实测常年为空
//! （`memories` 表 0 行）；项目层的事实库里明明有行，模型也**看不见**——它得先
//! 想到要调 `memory_list` 才知道本项目记了什么。MR 稿 §0 那条真实失败的第三条
//! 根因就是这个：「注入面三重盲」。本件把**指针级索引**放进 system，让新会话
//! 第一句话之前就知道"这个项目记了哪些条"。
//!
//! **注入的是指针不是正文**（与 v2.136「项目层事实不自动注入」裁定不冲突，
//! 那条禁的是把事实全文灌进 system）：一行一条 `- <key> — <hook>`，正文仍按需
//! `memory_read`。这与 ZCode 的 `MEMORY.md` 常驻索引同构（MR 稿 §1.2）。
//!
//! 两条数据源，**md 投影优先**（人可编辑 = 人类层是真源的延续）：
//! 1. `<数据根>/memory/projects/<项目目录>/MEMORY.md` 的指针行；
//! 2. md 缺失**或有文件但一行指针都没有**（2026-10-12 本机实测：`TITY` 项目目录
//!    里根本没有 `MEMORY.md`，另外三个项目的都只有 `# 记忆索引` 空壳标题）⇒
//!    由 sqlite 该键位的行现合成，**不回写文件**（稿 §3 真源声明）。
//!
//! 预算（稿 §6）：200 行 / 25,000 字符，超限截断并附一行 WARNING 指路
//! `memory_list` / `memory_search`。陈旧标注 `（N 天前）` 由注入侧按 `updated_at`
//! 现算，>48 h 才标，**不写进文件**（新鲜度只有 mtime/updated_at 一个信号，
//! 稿 §2 纪律 5）。
//!
//! 字节钉死不在本件里做：整块随 [`crate::memory`] 的 `MemorySource` 交付，
//! 吃 `MemoryInject` 既有的会话预取缓存（键 = `(session_id, scope_id)`）。

use crate::memory::FactLine;

/// 索引预算的**唯一出处在 `plugin_api::memlayout`**（写侧落盘的 `MEMORY.md` 与
/// 注入侧共用同一份数字——两头各写一份迟早会漂成"文件里 210 行、注入只给看 200 行"）。
/// 这里只做再导出，让本模块与它的测试继续按裸名取用。
pub(crate) use plugin_api::memlayout::{MAX_INDEX_CHARS, MAX_INDEX_LINES};
/// 陈旧标注门槛：48 h（稿 §6「>48 h 才标」）。
pub(crate) const STALE_AFTER_SECS: i64 = 48 * 3600;

/// 索引块导语（给模型的说明，中英与 `<persistent_memory>` 同档英文）。
const LEAD_IN: &str = "Project memory index — pointers only, never the bodies (one line \
     per saved fact of THIS project). Read a value with memory_read(key=…), search with \
     memory_search. Empty results here mean this project has no saved facts, not that it \
     has no history — for past work use memory_project_history.";

/// 超限时的指路行（截断必须自描述，否则模型以为"就这些"）。
const TRUNCATED_NOTE: &str = "… index truncated — use memory_list or memory_search for the \
     remaining entries";

/// 由 `MEMORY.md` 投影合成索引内文（**优先档**，人可编辑）。
///
/// 返回 `None` = 这份 md 用不上（文件化未接线 / 读不到 / 只有标题 / 一行指针都
/// 没有）——2026-10-12 本机实测就是这个形态（`TITY` 项目目录里没有 `MEMORY.md`，
/// 另外三个项目的都只有 `# 记忆索引` 空壳），所以调用方**必须**接着试兜底档，
/// 不许把"文件存在"当成"索引有货"。
pub(crate) fn from_md(md_body: Option<&str>) -> Option<String> {
    finish(pointer_lines_from_md(md_body)?)
}

/// 由 sqlite 该键位的行现合成（**兜底档**，不回写文件——稿 §3 真源声明）。
pub(crate) fn from_rows(rows: &[FactLine], now: i64) -> Option<String> {
    let lines = rows
        .iter()
        .map(|r| {
            // `- <key> — <tags>（N 天前）`（稿 §4.1 原文形状；无 tags 就不留空钩子）
            let hook = one_line(&r.tags);
            let stale = stale_marker(r.updated_at, now);
            if hook.is_empty() {
                format!("- {}{stale}", r.key)
            } else {
                format!("- {} — {hook}{stale}", r.key)
            }
        })
        .collect::<Vec<_>>();
    finish(lines)
}

/// 共用的收尾：导语 + 钳制 + 截断指路（两档走同一条，钳制口径不会分叉）。
fn finish(lines: Vec<String>) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let (kept, truncated) = clamp(lines);
    let mut out = String::from(LEAD_IN);
    for l in kept {
        out.push('\n');
        out.push_str(&l);
    }
    if truncated {
        out.push('\n');
        out.push_str(TRUNCATED_NOTE);
    }
    Some(out)
}

/// 从 `MEMORY.md` 里取指针行（`- [Title](file.md) — hook` 形态，与
/// `plugin_api::memlayout::refresh_index` 的写出格式同源——**照生产构造代码抄**）。
///
/// `None` = 一个指针行都没有（只有标题 / 全是注释 / 空文件 / 读不到）。
fn pointer_lines_from_md(md_body: Option<&str>) -> Option<Vec<String>> {
    let body = md_body?;
    let mut out = Vec::new();
    let mut in_html_comment = false;
    for raw in body.lines() {
        let line = raw.trim_end();
        // 剥 HTML 注释（人写的批注不许进上下文，稿 §1.1 同款清洗）
        if in_html_comment {
            if line.contains("-->") {
                in_html_comment = false;
            }
            continue;
        }
        if line.starts_with("<!--") {
            if !line.contains("-->") {
                in_html_comment = true;
            }
            continue;
        }
        if line.starts_with("# ") || line.trim().is_empty() {
            continue;
        }
        if line.starts_with("- ") {
            out.push(line.to_string());
        }
    }
    (!out.is_empty()).then_some(out)
}

/// 行数与字符双钳：任一超限即截断并附 WARNING。
fn clamp(mut lines: Vec<String>) -> (Vec<String>, bool) {
    let mut truncated = false;
    if lines.len() > MAX_INDEX_LINES {
        lines.truncate(MAX_INDEX_LINES);
        truncated = true;
    }
    let mut budget = MAX_INDEX_CHARS;
    let mut kept: Vec<String> = Vec::with_capacity(lines.len());
    for l in lines {
        let cost = l.chars().count() + 1; // +1 = 换行
        if cost > budget {
            truncated = true;
            break;
        }
        budget -= cost;
        kept.push(l);
    }
    (kept, truncated)
}

/// 陈旧标注（>48 h 才标；未来时间戳 = 不标，不发明负数天数）。
fn stale_marker(updated_at: i64, now: i64) -> String {
    let age = now.saturating_sub(updated_at);
    if age <= STALE_AFTER_SECS {
        return String::new();
    }
    let days = age / 86_400;
    format!("（{days} days old）")
}

/// 单行化 + 截断（与 `memlayout::one_line` 同纪律，这里自带一份以免跨模块
/// 依赖把"注入面"和"文件层"耦在一起）。
fn one_line(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = flat.trim();
    if trimmed.chars().count() > 80 {
        format!("{}…", trimmed.chars().take(80).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests;
