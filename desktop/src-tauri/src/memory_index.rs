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
//! 两档数据源，S1 刀②b-3 起**判据是"人类层有没有接线"**，不是"md 像不像有货"：
//! 1. 接线 ⇒ **只读** `<数据根>/memory/projects/<项目目录>/MEMORY.md` 的指针行；文件缺失
//!    或只有 `# 记忆索引` 空壳标题（2026-10-12 本机实测就是这个形态：`TITY` 目录里根本没有
//!    `MEMORY.md`，另外三个项目都只有空壳）⇒ **整节如实缺席**。真源就是文件，
//!    "文件里没有指针行"等于"这个项目没记过"，不许再去库里补一份已不再是真源的副本。
//! 2. 未接线（单测 / 降级启动）才由 sqlite 该键位的行现合成，**不回写文件**——
//!    这一档是暂时状态，刀②c 连同 `index_rows` 一起删。
//!
//! 预算（稿 §6）：200 行 / 25,000 字符，与写侧**同一份常量**（`memlayout`，SS10 三档盯着），
//! 超限附的那句 WARNING 也与写侧**同一串**；文件里本来就带着这句时（写侧钳出来的）
//! 这里要**认出来并透传**——`pointer_lines_from_md` 只收指针行，先前会把这句剥掉，
//! 于是"人打开文件夹看到还有更多、模型拿到的却看起来完整"。陈旧标注 `（N 天前）`
//! 由注入侧按 `updated_at` 现算，>48 h 才标，**不写进文件**（新鲜度只有 mtime/updated_at
//! 一个信号，稿 §2 纪律 5）。
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
///
/// 刀②b-3 起它的**值 = 写侧那一句**（`memlayout::INDEX_TRUNCATED_NOTE`）：原先
/// "文件里写一句给人看、注入里另写一句给模型看"= 同一件事两个出处，正是这一刀要
/// 消掉的暂时状态。裸名 `TRUNCATED_NOTE` 保留，本模块与它的测试继续按旧名取用。
const TRUNCATED_NOTE: &str = plugin_api::memlayout::INDEX_TRUNCATED_NOTE;

/// 由 `MEMORY.md` 投影合成索引内文（S1 刀②b-3 起这是**人类层可用时唯一的一档**）。
///
/// 返回 `None` = 这一节缺席（文件化未接线 / 读不到 / 只有标题 / 一行指针都没有）。
/// 刀②b-3 把两档的关系**反过来**了：原先"md 空壳就退回库里现合成"是双写时代的
/// 人可编辑优先；md 成真源之后，"文件里没指针行"就是**这个项目确实没记过**，
/// 如实缺席，不再去库里找一份已经不再是真源的副本。
///
/// 另外这里要把写侧那句截断说明**认出来并交给 `finish`**：`pointer_lines_from_md`
/// 只收 `- ` 开头的指针行，那句 WARNING 会被剥掉 ⇒ 若不当标记处理，索引被截断这件事
/// 到模型那一侧就消失了（正是"人打开文件夹看到截断、模型看不到"的那种分叉）。
pub(crate) fn from_md(md_body: Option<&str>) -> Option<String> {
    let body = md_body?;
    let truncated_by_file = body.lines().any(|l| l.trim_end() == TRUNCATED_NOTE);
    finish(pointer_lines_from_md(Some(body))?, truncated_by_file)
}

/// 由 sqlite 该键位的行现合成（**兜底档**，不回写文件——稿 §3 真源声明）。
///
/// 刀②b-3 之后这一档的**触发条件变了**：不再是"md 空壳就退到这里"，而是"人类层
/// 压根没接线"（单测与降级启动）。md 是真源，空壳文件就是"这个项目没记过"。
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
    finish(lines, false)
}

/// 共用的收尾：导语 + 钳制 + 截断指路。
///
/// `truncated_by_file` = 文件里本来就带着那句截断说明（写侧钳出来的）。它和
/// "注入侧这一轮又钳了一次"**取并集**：两条都必须写那句话，否则——只认注入侧那一半
/// 的话，文件被写侧截断而注入侧没超限时，模型看到的是一份**看起来完整**的索引。
/// 钳制本身留着不是冗余：人可以把 `MEMORY.md` 手工编辑成超出预算的形状，
/// 预算保护必须站在注入通路上（数字与文案都只有一个出处 = `memlayout`）。
fn finish(lines: Vec<String>, truncated_by_file: bool) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let (kept, clamped) = clamp(lines);
    let truncated = clamped || truncated_by_file;
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
