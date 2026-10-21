//! 门槛算式与保留区切点（specs/13 刀①）。
//!
//! 逐字来源 = 上游 `packages/core/src/session/compaction.ts`：
//! 门槛（`:849-856` `calculateCeiling`）与保留区切点（`:718-760`
//! `splitConversation` / `recentStart` / `oldestToDrop`）。
//!
//! **上游的门槛算式住在压缩层里**（`window − max(window×0.15, 16k)`），
//! 本仓把它换成「用户百分比 + 输出预留」：`min(窗口 × 百分比, 窗口 − 输出预留)`
//! ——左半是用户意图，右半是物理约束（窗口封顶）。

use kernel_agent::estimate_tokens;
use session_store::Message;

/// 门槛算式（specs/13 · C1）：`min(窗口 × 用户百分比, 窗口 − 输出预留)`。
///
/// `pct` 是用户那一格（1–100）；`window` 必须是**已知**的窗口（0 = 未知 ⇒
/// 调用方不许拿它算门槛——窗口未知时自动档**不触发**，同上游 A2）。
///
/// 预留量那一半**不重算**：走 `context_budget::effective_threshold`（输出预留的
/// 唯一出处），免得"压缩面算一遍、进度条算一遍"。
pub(crate) fn ceiling(window: u64, pct: u64, max_output: Option<u64>) -> u64 {
    let slider = window.saturating_mul(pct.clamp(1, 100)) / 100;
    crate::context_budget::effective_threshold(
        slider,
        Some(window),
        max_output,
        crate::context_budget::OUTPUT_RESERVE_CAP_TOKENS,
    )
}

/// 输出侧预留量（token）= 窗口 − 封顶后的门槛。
///
/// 为什么绕一圈算：预留量的公式住在 `context_budget`（私有），而压缩面要的是
/// "窗口里还能塞多少"（上游 `summaryRoom`）——用同一个函数反推，两边不会漂。
pub(crate) fn summary_room(window: u64, max_output: Option<u64>) -> u64 {
    let capped = crate::context_budget::effective_threshold(
        window,
        Some(window),
        max_output,
        crate::context_budget::OUTPUT_RESERVE_CAP_TOKENS,
    );
    window.saturating_sub(capped)
}

/// 这条消息是不是 checkpoint（上游 `previousCompaction` 判的就是"第一条是不是
/// 一次压完的压缩"）。
pub(crate) fn is_checkpoint(m: &Message) -> bool {
    matches!(m, Message::User { content, .. } if content.starts_with(session_store::CHECKPOINT_TAG))
}

/// 上一轮压过且**留了原文**（上游 `kept(previous)` 非空）。
///
/// 那一档的语义：只压摘要之前的那一段，原文整段留着。
fn kept_previous(messages: &[Message]) -> bool {
    messages
        .first()
        .is_some_and(|m| is_checkpoint(m) && message_text(m).contains("<recent-context>"))
}

/// 消息在保留区里的文本（`session-store` 的唯一出处）。
fn message_text(m: &Message) -> String {
    session_store::message_to_text(m)
}

/// 保留区切点（上游 `compaction.ts:718-760` 同款）。
///
/// 返回**保留指针** = 保留区在消息列里的起点（第一条被原样保留的消息下标）。
/// `None` = 没有可摘的内容（判据 `nothing-to-compact`）。
///
/// 三条规矩（上游逐条）：
/// 1. 切点落在**user 消息边界**（不把一次往返劈开）；
/// 2. 至少留最新一条；
/// 3. 全都装得下时只留最新一次往返（不把整段历史原样重复一份）。
pub(crate) fn split_conversation(messages: &[Message], keep: u64) -> Option<usize> {
    if messages.is_empty() {
        return None;
    }
    // 上游的条目 = "渲染得出非空文本的消息"；本仓每条消息都渲染得出非空文本
    // （`[User]: ` / `[Assistant]: ` / `[Tool result]: …`）⇒ 条目 = 消息。
    let entries: Vec<(usize, u64)> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| (i, estimate_tokens(std::slice::from_ref(m))))
        .collect();
    // 从最新往回装，装不下的那些最旧的消息先出列（上游 `oldestToDrop`）。
    let dropped = oldest_to_drop(&entries, keep).min(entries.len() - 1);
    // 切点推到 user 边界：assistant 的工具调用与结果要待在一起。
    if let Some(at) = entries
        .iter()
        .enumerate()
        .rev()
        .find(|(k, (i, _))| *k <= dropped && is_user(&messages[*i]))
        .map(|(k, _)| k)
    {
        if at > 0 {
            return Some(entries[at].0);
        }
    }
    // 全都装得下 ⇒ 只留最新一次往返（别把整段历史原样重复一份）。
    if let Some(at) = entries.iter().rposition(|(i, _)| is_user(&messages[*i])) {
        if at > 0 {
            return Some(entries[at].0);
        }
    }
    // 一轮都凑不齐：上一轮留了原文就全留（只压摘要之前那部分），否则全摘。
    Some(if kept_previous(messages) {
        0
    } else {
        messages.len()
    })
}

/// 最旧的那几条要让位（上游 `oldestToDrop`）：从尾部往前装，装不下的那些最旧的
/// 先出列。返回"第一条保住的条目下标"。
fn oldest_to_drop(entries: &[(usize, u64)], budget: u64) -> usize {
    let mut total = 0u64;
    let mut start = entries.len();
    while start > 0 {
        let next = total + entries[start - 1].1;
        if next > budget {
            break;
        }
        total = next;
        start -= 1;
    }
    start
}

fn is_user(m: &Message) -> bool {
    matches!(m, Message::User { .. })
}
