//! 文本截断唯一入口（《AUDIT-FIX-PLAN-v1》批 F·F2 收敛，审计 M-1 同族）。
//!
//! 收敛前有 **6 个模块各写一份同名的 `clip`**，实际是 **4 种互不相同的行为**：同名
//! 不同物正是「改一处漏一处 + 顺手误用」的产地（v2.133 的 ENV_LOCK 分裂、v2.135 的
//! gc 序错都是同源实现分家后的产物）。本模块的做法 = **每种语义一份实现、名字说清
//! 是哪一种**，六处调用行为逐字节保持与收敛前一致（等值判据见 `text/tests.rs`：把
//! 被删的六份原文当参考实现逐边界比对，不是手写期望值）——`clip_markdown` 那条尤其
//! 要紧，它的输出是用户手里的 Markdown 文件字节，不是内部展示。
//!
//! | 语义 | 函数 | 现役调用面 |
//! |---|---|---|
//! | 纯截断，超长**不加**任何标记 | [`clip_plain`] | 浏览器控制台行文本、通知正文标签 |
//! | 超长才补一个 `…` | [`clip_ellipsis`] | 章节标题检索摘要、子 Agent 验收转录 |
//! | 先 `trim` 再纯截断 | [`clip_trimmed`] | 定时任务的标题/描述回显 |
//! | 截断 + `…（截断）` + 换行折平 | [`clip_markdown`] | 会话导出 Markdown（逐字节契约） |
//!
//! 计数一律按 `char` 而非字节：CJK 下 `&s[..n]` 会切在字符中间直接 panic。

/// 取前 `n` 个字符，超长不追加任何标记（展示面宽度控制用）。
pub(crate) fn clip_plain(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 取前 `n` 个字符；只在**真的超出**时补一个省略号（未超长原样返回，连尾随
/// 空白都不动——调用方拿它当「短形展示」而不是「改写」）。
pub(crate) fn clip_ellipsis(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        clip_plain(s, n) + "…"
    }
}

/// 先去掉首尾空白再取前 `n` 个字符。定时任务的回显面历史上就带这层 `trim`：
/// 模型传来的标题/描述常尾随换行，去掉才算"用户看得懂的一行"。
pub(crate) fn clip_trimmed(s: &str, n: usize) -> String {
    clip_plain(s.trim(), n)
}

/// 导出件专用截断：取前 `n` 个字符，超长补 `…（截断）`，并把内部换行折成空格
/// ——导出是一条 Markdown 引用行，裸换行会把引用打断。
pub(crate) fn clip_markdown(s: &str, n: usize) -> String {
    let mut out = clip_plain(s, n);
    if s.chars().count() > n {
        out.push_str("…（截断）");
    }
    out.replace('\n', " ")
}

#[cfg(test)]
mod tests;
