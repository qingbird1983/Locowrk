//! 压缩请求的**发送面**（specs/13 刀③）：**口 + 发 + 验货**。
//!
//! 这一半是"发出去之后发生的事"：**口**（[`CompactionLlm`] 那个可注入的发送口
//! ——主对话轮的 `LlmService` 契约里没有"带预算 + 不推重试事件 + delta 走自己
//! 回调"这三样，没有这一层就没法在测试里换掉网络）→ **发**一趟 → **验货**
//! （不合格追加一次提醒重发）→ 把供应商错误归到判据上。它有自己的失败分类表
//! （[`Failure`]），而本体（`deep.rs`）管的是"要不要发、瞄多大"——两者动一处常常
//! 要连着动（改判据要改分类表，改分类表要改本体的分支）⇒ 放一处那个文件撑不住
//! 400 行。
//!
//! **失败怎么归档（specs/13 刀⑦b）**：按**档位**（[`FailureKind`]），**不猜文案**
//! ——传输层在知道事实的地方（HTTP 状态码 / `finish_reason`）分好档，档位随错误
//! 一起上来。原来这里有三份文本判据（`is_output_limit` / `is_content_filter` /
//! `is_payload_too_large`），供应商换个措辞就归错档 ⇒ 随本刀删除。
//!
//! 逐字来源 = 上游 `packages/core/src/session/compaction.ts:293-316`（提醒一次）
//! 与 `:544-580`（失败分类）。

use std::time::{Duration, Instant};

use async_trait::async_trait;
use kernel_agent::{LlmRequest, LlmResponse, LlmUsage};
use session_store::Message;

use crate::compact::outcome::{
    BUDGET, CONTENT_FILTER, INTERRUPTED, MSG_CONTENT_FILTER, MSG_INTERRUPTED, MSG_NO_SUMMARY,
    MSG_OUTPUT_LIMIT, MSG_TEMPLATE_MISMATCH, NO_SUMMARY, OUTPUT_LIMIT, PROVIDER_ERROR,
    TEMPLATE_MISMATCH,
};
use crate::compact::prompt;
use crate::llm::failure::{FailureKind, LlmCallError};

use super::{Compactor, Hooks};

/// 压缩请求的发送口（**可注入**：测试面塞假件，组装点接 [`crate::llm::AppLlm`]）。
///
/// 为什么要一层 trait 而不是直接拿 `AppLlm`：主对话轮的 `LlmService` 契约里
/// 没有"带预算 + 不推重试事件 + delta 走自己回调"这三样（那是压缩独有的），
/// 而没有这一层就没法在测试里换掉网络。
///
/// 错误类型是 [`LlmCallError`]（**带档位**，specs/13 刀⑦b）：压缩层按**档**认
/// 失败（撞输出上限 / 被内容过滤 / 字节太大 / 超窗 / 预算耗尽），**不看文案**
/// ——所以这个 trait 的错误类型是宿主本地的那一枚，不是内核的 `LlmError`。
#[async_trait]
pub(crate) trait CompactionLlm: Send + Sync {
    async fn complete(
        &self,
        req: LlmRequest,
        budget: Duration,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmCallError>;
}

#[async_trait]
impl CompactionLlm for crate::llm::AppLlm {
    async fn complete(
        &self,
        req: LlmRequest,
        budget: Duration,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmCallError> {
        self.complete_compaction(req, budget, on_delta).await
    }
}

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
    ///
    /// **按档位归类，不看文案**（specs/13 刀⑦b）：档位是传输层在**知道事实的
    /// 地方**分好的（HTTP 状态码 / `finish_reason` / 预算环），随错误一起上来。
    /// 文案只当"细节"往界面/日志带（`Failure::TooLong` / `PayloadTooLarge` /
    /// `Provider` 那几档带它），**不参与归类**——所以把文案换成任意串也不该
    /// 改变这一趟归到哪一档。
    fn classify(&self, err: LlmCallError, session_id: &str) -> Failure {
        if self.is_stopped(session_id) {
            return Failure::Interrupted;
        }
        match err.kind {
            FailureKind::Budget => Failure::Budget,
            FailureKind::PayloadTooLarge => Failure::PayloadTooLarge(err.message),
            FailureKind::ContextOverflow => Failure::TooLong(err.message),
            FailureKind::OutputLimit => Failure::OutputLimit,
            FailureKind::ContentFilter => Failure::ContentFilter,
            // 其余（限流 / 瞬时 / 其它致命）：如实当供应商错误。
            _ => Failure::Provider(err.message),
        }
    }
}

/// 剩下的预算（总时长预算 = 整趟共享，不是每发一次重置）。
fn remaining(deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_millis(1))
}
