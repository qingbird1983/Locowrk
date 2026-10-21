//! 压缩请求的**发送与验货**那一半（specs/13 刀③）。
//!
//! 这一半是"发出去之后发生的事"：发一趟 → 校验输出（不合格追加一次提醒重发）→
//! 把供应商错误归到判据上。它有自己的失败分类表（[`Failure`]）与三枚文本判据，
//! 而本体（`deep.rs`）管的是"要不要发、瞄多大"——两者动一处常常要连着动
//! （改判据要改分类表，改分类表要改本体的分支）⇒ 放一处那个文件撑不住 400 行。
//!
//! 逐字来源 = 上游 `packages/core/src/session/compaction.ts:293-316`（提醒一次）
//! 与 `:544-580`（失败分类）。

use std::time::{Duration, Instant};

use kernel_agent::{LlmError, LlmRequest, LlmUsage};
use session_store::Message;

use crate::compact::outcome::{
    BUDGET, CONTENT_FILTER, INTERRUPTED, MSG_CONTENT_FILTER, MSG_INTERRUPTED, MSG_NO_SUMMARY,
    MSG_OUTPUT_LIMIT, MSG_TEMPLATE_MISMATCH, NO_SUMMARY, OUTPUT_LIMIT, PROVIDER_ERROR,
    TEMPLATE_MISMATCH,
};
use crate::compact::prompt;

use super::{Compactor, Hooks};

/// 失败的几档（判据名之外的形状）。
pub(super) enum Failure {
    /// 供应商说"太长"（可收缩）。
    TooLong(String),
    /// 字节太大（改纯文本再发，不计一次拒绝）。
    PayloadTooLarge(String),
    /// 撞输出上限。
    OutputLimit,
    /// 被内容过滤。
    ContentFilter,
    /// 超出总时长预算。
    Budget,
    /// 被叫停。
    Interrupted,
    /// 模板不符。
    TemplateMismatch,
    /// 模型没吐出摘要。
    NoSummary,
    /// 其它供应商错误。
    Provider(String),
}

impl Failure {
    /// 归到（判据名, 细节文本）。
    pub(super) fn into_outcome(self) -> (&'static str, Option<String>) {
        match self {
            Failure::OutputLimit => (OUTPUT_LIMIT, Some(MSG_OUTPUT_LIMIT.to_string())),
            Failure::ContentFilter => (CONTENT_FILTER, Some(MSG_CONTENT_FILTER.to_string())),
            Failure::Budget => (BUDGET, Some(String::new())),
            Failure::TemplateMismatch => {
                (TEMPLATE_MISMATCH, Some(MSG_TEMPLATE_MISMATCH.to_string()))
            }
            Failure::NoSummary => (NO_SUMMARY, Some(MSG_NO_SUMMARY.to_string())),
            Failure::Provider(text) => (PROVIDER_ERROR, Some(text)),
            Failure::TooLong(text) => (PROVIDER_ERROR, Some(text)),
            Failure::PayloadTooLarge(text) => (PROVIDER_ERROR, Some(text)),
            Failure::Interrupted => (INTERRUPTED, Some(MSG_INTERRUPTED.to_string())),
        }
    }
}

impl Compactor {
    /// 发一趟并**验货**（上游 `compaction.ts:293-316`）。
    ///
    /// 提示词作为**最后一条 user 消息**追加（hooks 看到的是加它之前的请求）；
    /// 回复里没有模板标题行 ⇒ 追加一条 [`prompt::NUDGE`] 重发**一次**；再不过
    /// ⇒ 失败（文案逐字用上游那两句）。
    pub(super) async fn send(
        &self,
        request: LlmRequest,
        prompt: &str,
        deadline: Instant,
        session_id: &str,
        hooks: &Hooks<'_>,
        usage: &mut Vec<LlmUsage>,
    ) -> Result<String, Failure> {
        let on_delta = |delta: &str| -> bool {
            (hooks.on_delta)(delta);
            self.is_stopped(session_id)
        };
        let mut first = request.clone();
        first.messages.push(Message::user(prompt.to_string()));
        let reply = self
            .llm
            .complete(first, remaining(deadline), &on_delta)
            .await
            .map_err(|e| self.classify(e, session_id))?;
        usage.extend(reply.usage);
        if prompt::filled(&reply.content) {
            return Ok(reply.content);
        }
        let mut nudged = request;
        nudged.messages.push(Message::user(prompt.to_string()));
        nudged
            .messages
            .push(Message::user(prompt::nudge().to_string()));
        let retry = self
            .llm
            .complete(nudged, remaining(deadline), &on_delta)
            .await
            .map_err(|e| self.classify(e, session_id))?;
        usage.extend(retry.usage);
        if prompt::filled(&retry.content) {
            return Ok(retry.content);
        }
        Err(if retry.content.trim().is_empty() {
            Failure::NoSummary
        } else {
            Failure::TemplateMismatch
        })
    }

    /// 把一条模型错误归到判据上（上游 `compaction.ts:544-580` 的分类）。
    fn classify(&self, err: LlmError, session_id: &str) -> Failure {
        let msg = err.to_string();
        if self.is_stopped(session_id) {
            return Failure::Interrupted;
        }
        if crate::llm::is_budget_exhausted(&msg) {
            return Failure::Budget;
        }
        if is_payload_too_large(&msg) {
            return Failure::PayloadTooLarge(msg);
        }
        if crate::llm::is_overflow_message(&msg) {
            return Failure::TooLong(msg);
        }
        if is_output_limit(&msg) {
            return Failure::OutputLimit;
        }
        if is_content_filter(&msg) {
            return Failure::ContentFilter;
        }
        Failure::Provider(msg)
    }
}

/// 剩下的预算（总时长预算 = 整趟共享，不是每发一次重置）。
fn remaining(deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_millis(1))
}

/// 撞输出上限的判据（本仓的传输层把 `finish_reason=length` 翻成那句中文报错，
/// 这里认它的稳定片段）。
fn is_output_limit(msg: &str) -> bool {
    msg.contains("finish_reason=length") || msg.contains("最大输出上限")
}

/// 内容过滤的判据（传输层不暴露 finish_reason ⇒ 只能按供应商原文认）。
fn is_content_filter(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    [
        "content_filter",
        "content policy",
        "content_policy",
        "safety filter",
        "moderation",
    ]
    .iter()
    .any(|m| lower.contains(m))
}

/// 「字节太大」的判据（与"太长"是两件事：前者多半是内联媒体，改纯文本就能发）。
///
/// 为什么住在压缩层而不是模型接入面：模型接入面的 `retry.rs` 判的是"上下文超限"
/// （token 那一档），而这一档是 HTTP 413 那一族；这一刀的文件所有权不含
/// `llm/retry.rs`，故就地落在这里并写清出处。
fn is_payload_too_large(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    [
        "payload too large",
        "request entity too large",
        "body too large",
        "request too large",
    ]
    .iter()
    .any(|m| lower.contains(m))
}
