//! 主层压缩的**接线半**（specs/13 刀①–③）：三种触发 + 请求装配 + 收缩矩阵。
//!
//! 与主对话轮的关系只有一句话：**压缩请求 = 主对话那条管线的真实窗口**。
//! 装配走内核的 [`Kernel::prepare_request`]（`derive_messages → waterfall_pre →
//! tool_catalog → waterfall_request`，与主对话轮同一条链），发送走
//! [`crate::llm::AppLlm::complete_compaction`]（与主对话轮同一套模型解析）。
//! 提示词在**装配之后**追加 ⇒ hooks 看到的是加提示词前的请求（上游同款）。
//!
//! 三种触发（上游 `session/compaction.ts:53-57` + `runner/llm.ts:114-157`）：
//!
//! * [`Trigger::Auto`] —— step 边界，占用到门槛才压；
//! * [`Trigger::Manual`] —— `/压缩`，**自动压缩关着也照样能压**；
//! * [`Trigger::Overflow`] —— 供应商按"太长"拒收后压一次、重试一次。
//!
//! 失败处置：**auto 失败 = 整步失败**（由 runner 判）；**overflow 失败 = 不恢复**
//! （原始错误照常冒泡）；**manual 失败 = 记失败事件、主轮继续**。
//!
//! **这一族按职责切成三块**（400 行纪律）：
//!
//! * 本文件 —— 本体：要不要发、瞄多大、失败之后怎么办；
//! * [`send`] —— 发一趟并验货 + 失败分类（[`send::Failure`]）；
//! * [`settle`] —— 落账（用量 + `ContextCompacted` + 保留指针）。

pub(crate) mod send;
pub(crate) mod settle;

use std::sync::Arc;
use std::time::{Duration, Instant};

use kernel_agent::{Kernel, LlmUsage};
use session_store::Session;

use super::deliver::{self, Shrink};
use super::outcome::{
    Skip, CANNOT_SHRINK, INTERRUPTED, MSG_CANNOT_SHRINK, MSG_INTERRUPTED, MSG_NOTHING_TO_COMPACT,
    NOTHING_TO_COMPACT, PROVIDER_ERROR,
};
use super::plan;
use super::prompt;
use super::{is_fused, Failures, Outcome, Overflow, KEEP_TOKENS, SHRINK_STEPS, UNKNOWN_WINDOW};
use send::Failure;
use settle::{append_usage, just_compacted, without_tail};

/// 压缩请求的发送口（**可注入**：测试面塞假件，组装点接 [`crate::llm::AppLlm`]）。
///
/// 为什么住在 [`send`] 那一半（400 行纪律 · 刀⑦b）：发送面 = 「口 + 发 + 验货」
/// 是同一件事；本体（这个文件）管的是「要不要发、瞄多大、失败之后怎么办」。
/// 本体只留**名字**（`compact::deep::CompactionLlm` 那个路径不变）。
pub(crate) use send::CompactionLlm;

/// `ContextCompacted.strategy` 在这一层的取值（唯一出处：写侧与判据都读它）。
pub(crate) const STRATEGY: &str = "compact";

/// 这一趟的界面钩子（压缩层不认 UI，只管敲门；翻译成事件由 runner 那一层做）。
pub(crate) struct Hooks<'a> {
    /// 「这一趟真的开始压了」——判定通过、请求发出**之前**敲一次。
    pub on_start: &'a (dyn Fn() + Send + Sync),
    /// 摘要正文的流式增量（上游 `Compaction.Delta`）。
    pub on_delta: &'a (dyn Fn(&str) + Send + Sync),
}

/// 三种触发。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// 自动（step 边界）。
    Auto,
    /// 手动（`/压缩`）。
    Manual,
    /// 溢出（供应商按"太长"拒收之后）。
    Overflow,
}

/// 主层压缩的执行体（runner 在 step 边界调它，组装点构造它）。
pub(crate) struct Compactor {
    settings: Arc<crate::settings::SettingsStore>,
    /// 请求装配（**与主对话轮同一条链**）。
    kernel: Arc<Kernel>,
    /// 发送口。
    llm: Arc<dyn CompactionLlm>,
    /// 连续失败表（熔断的输入）。**由组装点注入**——两个使用方要读同一张账。
    failures: Arc<Failures>,
    /// 超窗拒收的读数：写它的是发送侧，本层只读 + 压成之后清。
    refusals: Arc<Overflow>,
    /// 停止标记：用户能停掉这一条看不见的辅助请求。
    stop: Option<Arc<crate::stop::StopFlags>>,
    /// 会话日志目录（`<数据根>/sessions`）：压缩这一族**自己落盘**用。
    ///
    /// 为什么压缩层要认这个目录（specs/13 刀⑥）：落盘口收成一处之后，"这一趟
    /// 追加的事件写进日志"这件事**由做事的那一层负责**——压缩层知道自己追加了
    /// 什么（用量 + 压缩落账），让它自己落，"跑"与"落盘"就是**一个调用**，
    /// 漏不了（改前手动那条漏过一次：压完看着一切正常，重开应用压缩连同摘要一起
    /// 消失）。三条触发（自动 / 手动 / 溢出）共用这一处 ⇒ 压缩不再"两处各落各的"。
    ///
    /// 落盘的起点是**会话自己的游标**（`Session::persisted`），不是谁临时算的
    /// `from` ⇒ 压缩层落完、runner 步末再落一次时是**空段**（游标已到末尾）
    /// ⇒ 不会重影。
    ///
    /// 改前这里有个 `run_manual_and_persist`（手动那条的"跑 + 落盘"合成一个调用）：
    /// 它的两个职责已分别在 `Compactor::run_manual` 与 `try_compact` 末尾落进同一个
    /// 调用里 ⇒ **退役**（留着就多一个"落盘口在两处"的错觉）。它漏过一次（手动压完
    /// 磁盘上还是压前那一份历史 ⇒ 重开应用压缩凭空消失），那次教训由
    /// `tests/compaction_opencode/persist.rs` 那条判据继续钉着。
    log: Option<std::path::PathBuf>,
}

impl Compactor {
    pub(crate) fn new(
        settings: Arc<crate::settings::SettingsStore>,
        kernel: Arc<Kernel>,
        llm: Arc<dyn CompactionLlm>,
        failures: Arc<Failures>,
        refusals: Arc<Overflow>,
        stop: Option<Arc<crate::stop::StopFlags>>,
        log: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            settings,
            kernel,
            llm,
            failures,
            refusals,
            stop,
            log,
        }
    }

    /// 占用读数（"到线了没"的分子）：与进度条同吃 `context_budget::occupancy`。
    pub(crate) fn used(&self, session: &Session, session_id: &str) -> u64 {
        crate::context_budget::occupancy(session, self.refusals.reading(session_id))
    }

    /// 该会话最近一次被供应商按超窗拒收时的占用读数（没有记录 = 0）。
    ///
    /// 读口留在这一个 struct 上：宿主（组装点）不必再多一个"谁都能碰那张表"的
    /// 字段，而这张表的主人是压缩层。
    pub(crate) fn refusal(&self, session_id: &str) -> u64 {
        self.refusals.reading(session_id)
    }

    /// 该会话是否已被请求停止。
    fn is_stopped(&self, session_id: &str) -> bool {
        self.stop.as_ref().is_some_and(|f| f.is_stopped(session_id))
    }

    /// 自动通路（step 边界）。
    pub(crate) async fn run(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &crate::runner::UsagePolicy,
        hooks: &Hooks<'_>,
    ) -> Outcome {
        self.try_compact(session, session_id, policy, Trigger::Auto, hooks)
            .await
    }

    /// 手动通路（`/压缩`）：**自动压缩关着也照样能压**（上游同款）。
    pub(crate) async fn run_manual(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &crate::runner::UsagePolicy,
        hooks: &Hooks<'_>,
    ) -> Outcome {
        self.try_compact(session, session_id, policy, Trigger::Manual, hooks)
            .await
    }

    /// 溢出通路：供应商按"太长"拒收之后压一次（上游 `runner/llm.ts:259-265`）。
    pub(crate) async fn run_overflow(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &crate::runner::UsagePolicy,
        hooks: &Hooks<'_>,
    ) -> Outcome {
        self.try_compact(session, session_id, policy, Trigger::Overflow, hooks)
            .await
    }

    /// 三条通路的共用主体。
    ///
    /// **末尾那一趟落盘**（specs/13 刀⑥）：这一趟追加的事件（压缩的用量 + 落账
    /// 那条 `ContextCompacted`；失败那几档也有用量）走**唯一那个落盘口**写进会话
    /// 日志——起点是会话自己的游标（`Session::persisted`），不是谁临时算的
    /// `from`。压缩层自己落 ⇒ 手动那条"跑 + 落盘"是一个调用（漏不了），
    /// 自动与溢出两条也走同一处 ⇒ 压缩不再"两处各落各的"。
    async fn try_compact(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &crate::runner::UsagePolicy,
        trigger: Trigger,
        hooks: &Hooks<'_>,
    ) -> Outcome {
        let out = self
            .try_compact_inner(session, session_id, policy, trigger, hooks)
            .await;
        if let Some(dir) = self.log.as_deref() {
            crate::session_log::persist_pending(dir, session_id, session);
        }
        out
    }

    /// 判定 → 发一趟 → 验货 → 落账（`try_compact` 的本体）。
    async fn try_compact_inner(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &crate::runner::UsagePolicy,
        trigger: Trigger,
        hooks: &Hooks<'_>,
    ) -> Outcome {
        let cfg = self.settings.compact_config();
        // 上游：只有人主动压才不受"自动压缩关着"影响（overflow 也算自动那一档）。
        if trigger != Trigger::Manual && !cfg.enabled {
            return Outcome::Skipped(Skip::Disabled);
        }
        let window = policy.window_tokens;
        let known = window > 0;
        let used = self.used(session, session_id);
        if trigger == Trigger::Auto {
            // 窗口未知 ⇒ **不自动触发**（上游 A2：靠 overflow 兜底）。
            if !known {
                return Outcome::Skipped(Skip::BelowLine);
            }
            if just_compacted(session) {
                return Outcome::Skipped(Skip::JustCompacted);
            }
            if used < plan::ceiling(window, cfg.auto_compact_pct, policy.max_output) {
                return Outcome::Skipped(Skip::BelowLine);
            }
            // 熔断：连败到线 ⇒ 自动压缩停手（相位 `paused`）。
            if is_fused(self.failures.of(session_id)) {
                return Outcome::Paused;
            }
        }
        // 摘要的提示词：已有 checkpoint ⇒ 更新变体（上游 `buildPrompt(update)`）。
        let messages = session.derive_messages();
        let Some(keep_from) = plan::split_conversation(&messages, KEEP_TOKENS) else {
            return Outcome::Failed {
                reason: NOTHING_TO_COMPACT,
                detail: Some(MSG_NOTHING_TO_COMPACT.to_string()),
            };
        };
        let prompt = prompt::build_prompt(messages.first().is_some_and(plan::is_checkpoint), false);
        // 装配：与主对话轮同一条链（hooks 看到的是**加提示词前**的请求）。
        let mut request = match self.kernel.prepare_request(session, Some(session_id)).await {
            Ok(r) => r,
            Err(e) => {
                return Outcome::Failed {
                    reason: PROVIDER_ERROR,
                    detail: Some(e.to_string()),
                }
            }
        };
        // 辅助调用标记：思考降最低档、不发 max_tokens（§5.4）。
        if !request.params.is_object() {
            request.params = serde_json::json!({});
        }
        request.params["aux"] = serde_json::Value::Bool(true);
        // 「这一趟真的开始压了」：判定通过、请求发出之前（手动那一格不敲——界面自己
        // 落了「正在压缩」那条线，后端再发一次就是两根同义的线）。
        if trigger != Trigger::Manual {
            (hooks.on_start)();
        }
        let (target, fit) = self.sizes(trigger, policy, cfg.auto_compact_pct, used, window, known);
        let history = without_tail(&request, keep_from);
        let mut shrink = Shrink::new(target, fit);
        let deadline = Instant::now() + Duration::from_millis(cfg.budget_ms);
        let mut usage: Vec<LlmUsage> = Vec::new();
        loop {
            let Some(candidate) = deliver::pick(&request, &history, &shrink) else {
                append_usage(session, &usage);
                return Outcome::Failed {
                    reason: CANNOT_SHRINK,
                    detail: Some(MSG_CANNOT_SHRINK.to_string()),
                };
            };
            let estimate = deliver::estimate_request(&candidate.request);
            match self
                .send(
                    candidate.request,
                    &prompt,
                    deadline,
                    session_id,
                    hooks,
                    &mut usage,
                )
                .await
            {
                Ok(summary) => {
                    return self
                        .settle(session, session_id, &messages, keep_from, &summary, &usage);
                }
                Err(err) => {
                    match err {
                        Failure::TooLong(text) => match shrink.on_too_long(estimate) {
                            Some(step) => {
                                tracing::warn!(
                                    session = session_id,
                                    step,
                                    estimate,
                                    "压缩请求超窗 ⇒ 按上游递减带收窄后再试"
                                );
                                continue;
                            }
                            None => {
                                append_usage(session, &usage);
                                return Outcome::Failed {
                                    reason: PROVIDER_ERROR,
                                    detail: Some(text),
                                };
                            }
                        },
                        Failure::PayloadTooLarge(text) => {
                            if !candidate.flattened && !shrink.as_text {
                                // 字节太大（多半是内联媒体）⇒ 同一请求改纯文本再发，
                                // **不计一次拒绝**（上游 `compaction.ts:448-453`）。
                                shrink.as_text = true;
                                continue;
                            }
                            append_usage(session, &usage);
                            return Outcome::Failed {
                                reason: PROVIDER_ERROR,
                                detail: Some(text),
                            };
                        }
                        Failure::Interrupted => {
                            append_usage(session, &usage);
                            // 叫停**不是失败**：不推进熔断计数（停过三次不该让压缩永久停手）。
                            return Outcome::Failed {
                                reason: INTERRUPTED,
                                detail: Some(MSG_INTERRUPTED.to_string()),
                            };
                        }
                        other => {
                            append_usage(session, &usage);
                            let (reason, detail) = other.into_outcome();
                            self.failures.record(session_id);
                            return Outcome::Failed { reason, detail };
                        }
                    }
                }
            }
        }
    }

    /// 这一趟瞄多大（上游 `compaction.ts:224-231` + `:324`）。
    ///
    /// * 自动 / 手动：目标 = 门槛；"装得下"的尺 = `max(门槛, 窗口 − 输出预留)`
    ///   （上游 `summaryRoom`：自动压缩已经越过门槛，整段原样发还装得下摘要）。
    /// * 溢出：目标 = `min(门槛, 估算 × 70%)`——供应商刚拒收过，估算偏低。
    fn sizes(
        &self,
        trigger: Trigger,
        policy: &crate::runner::UsagePolicy,
        pct: u64,
        used: u64,
        window: u64,
        known: bool,
    ) -> (u64, u64) {
        let base = if known { window } else { UNKNOWN_WINDOW };
        let cap = plan::ceiling(base, pct, policy.max_output);
        match trigger {
            Trigger::Overflow => {
                let t = cap.min((used as f64 * SHRINK_STEPS[0]).floor() as u64);
                (t, t)
            }
            _ => {
                let room = plan::summary_room(base, policy.max_output);
                (cap, cap.max(room))
            }
        }
    }
}
