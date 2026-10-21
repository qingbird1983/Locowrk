//! 压缩压成之后的**落账**那一半（specs/13 刀②–③）：用量记账 + `ContextCompacted`
//! 事件 + 保留指针。
//!
//! 为什么单独成文件：落账这一半动的是**会话事件的形状**（`ContextCompacted` 带
//! 保留指针、折叠产出一条 checkpoint），而本体（`deep.rs`）动的是"要不要发、瞄多大"。
//! 两者互不相干却互相牵连（改事件字段要同步改折叠侧）⇒ 各自成文件。

use kernel_agent::{estimate_tokens, LlmRequest, LlmUsage};
use session_store::{Message, Session, SessionEvent};

use crate::compact::Outcome;

use super::{Compactor, STRATEGY};

impl Compactor {
    /// 压成 ⇒ 落账（用量 + `ContextCompacted`）。
    pub(super) fn settle(
        &self,
        session: &mut Session,
        session_id: &str,
        messages: &[Message],
        keep_from: usize,
        summary: &str,
        usage: &[LlmUsage],
    ) -> Outcome {
        let kept = &messages[keep_from..];
        let checkpoint =
            session_store::checkpoint_message(summary, &session_store::recent_text(kept));
        let from_tokens = estimate_tokens(messages);
        let to_tokens = estimate_tokens(std::slice::from_ref(&checkpoint));
        // 用量**先**落、压缩事件**后**落：`context_budget::occupancy` 靠这个次序
        // 认出"那条上报量的是压缩之前的窗口"⇒ 压完之后分子重新走估算（v2.138
        // 「看到的 = 触发用的」）。
        append_usage(session, usage);
        session.append(SessionEvent::ContextCompacted {
            kept_messages: kept.len(),
            keep_from,
            summary: summary.to_string(),
            from_tokens,
            to_tokens,
            strategy: STRATEGY.to_string(),
            ts: crate::timeutil::now_ms(),
        });
        self.failures.clear(session_id);
        self.refusals.clear(session_id);
        Outcome::Compacted {
            kept_messages: kept.len(),
            from_tokens,
            to_tokens,
        }
    }
}

/// 把这一趟的用量记进会话（**失败 / 中断也记**，上游 `compaction.ts:628-639`）。
pub(super) fn append_usage(session: &mut Session, usage: &[LlmUsage]) {
    if usage.is_empty() {
        return;
    }
    let total = usage.iter().fold(LlmUsage::default(), |mut acc, u| {
        acc.input += u.input;
        acc.output += u.output;
        acc.cache_read += u.cache_read;
        acc.cache_write += u.cache_write;
        acc.reasoning += u.reasoning;
        acc.total += u.total;
        acc
    });
    session.append(SessionEvent::LlmUsage {
        input: total.input,
        output: total.output,
        cache_read: total.cache_read,
        cache_write: total.cache_write,
        reasoning: total.reasoning,
        total: total.total,
        duration_ms: 0,
    });
}

/// 去掉保留区的那一段（上游 `withoutTail`）。
pub(super) fn without_tail(request: &LlmRequest, keep_from: usize) -> LlmRequest {
    let cut = keep_from.min(request.messages.len());
    LlmRequest {
        messages: request.messages[..cut].to_vec(),
        ..request.clone()
    }
}

/// 上一条是不是一次压完的压缩（上游 `due()` 的第一条守卫）。
pub(super) fn just_compacted(session: &Session) -> bool {
    matches!(
        session.events().last(),
        Some(SessionEvent::ContextCompacted { .. })
    )
}
