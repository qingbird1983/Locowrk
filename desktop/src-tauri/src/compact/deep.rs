//! 主层深度压缩的**接线半**（S2 刀④c）：把 [`super::plan`] 的判定变成一次模型往返
//! 加一条日志事件。
//!
//! 判据与执行分两个文件不是为了"分层好看"：判定那一半是纯函数（`compact.rs` +
//! [`super::prompt`]，没有 IO、没有设置、没有会话），所以 AC-S2-6 的"切在哪一条、
//! 缺哪一节"能脱离桌面端跑；这一半才有 `session.append` 与网络。为什么调用点在这里
//! 而不是 `agent/request` 环上 = `compact.rs` 头注那三条（重试烧 N 份 / 失败丢账 /
//! 环上拿不到锚点这把尺）。
//!
//! 这一刀真正要小心的四件事，各在下面的对应位置展开：
//! ① [`Compactor::used`] 的**新鲜度**——压完之后 `LlmUsage` 还是压之前那个数，
//! 不换尺就会每个 step 再烧一条摘要往返；
//! ② **先验货再落账**——验的是归一化之后那一份文本（模型把整份摘要包进代码块、
//! 剥完什么都不剩，也算不合格），不合格只进失败计数（熔断的输入），不进日志；
//! ③ `to_tokens` 用**实测**（摘要文本此刻才存在），与判定时的目标位分属"预估"与
//! "落账实测"，不是同一读数的两个出处；
//! ④ **overflow 递减只认超窗**（刀④d）：判别用 `llm::retry` 那一份，一次压缩无论
//! 试了几档都只算一个失败计数——把 4 趟算成 4 次失败，等于一轮就把这一层锁死。

use std::sync::Arc;

use kernel_agent::estimate_tokens;
use memory_tree::MemoryManagerClient;
use session_store::{summarized_message, Session, SessionEvent};

use super::{
    plan,
    prompt::{build_prompt, format_summary, missing_sections, render_history, shrink_history},
    Failures, PlanInput, SHRINK_STEPS,
};
use crate::runner::UsagePolicy;
use crate::settings::SettingsStore;

/// `ContextCompacted.strategy` 在这一层的取值（唯一出处：写侧与判据都读它，
/// 不许在第二处敲同一个字面量）。
pub(crate) const STRATEGY: &str = "compact";

/// 一次尝试的读数。四个变体各对应"界面上该说什么"——判据不许靠"返回 0"
/// 反推是哪一步拦的（`organize::Release::Skip` 同一条纪律）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// 开关关掉 = 这一层整格不动（唯一一条不看读数的退出）
    Off,
    /// 判据说不动：`reason` = [`super::plan`] 那六条之一
    Skipped(&'static str),
    /// 调了模型但没成：`why` = `summary-call`（普通失败）/ `overflow`（缩到最小一档
    /// 还是超窗）/ `rejected`（摘要不合格），`failures` = 计入后的累计数，
    /// `attempts` = 这一次共发了几条往返（递减重试要能在观测面上看见花了几趟）。
    Failed {
        why: &'static str,
        failures: u32,
        attempts: u32,
    },
    /// 压成并落了账
    Compacted {
        kept_messages: usize,
        from_tokens: u64,
        to_tokens: u64,
    },
}

/// 主层压缩的执行体（runner 在 step 边界调它，组装点构造它）。
pub(crate) struct Compactor {
    settings: Arc<SettingsStore>,
    /// 摘要走**辅助补全的直呼通路**（`llm/aux_client.rs`，与整理章节总结同一条）：
    /// 压缩不是本轮对话的一部分，不该带工具目录、也不该吃会话级模型覆盖。
    aux: Arc<dyn MemoryManagerClient>,
    /// 连续失败表（熔断的输入）。**由组装点注入**而不是自己 new 一枚：滑窗保护网
    /// （`slide.rs`）要读同一个熔断态，两张表 = "主层已熔断但保护网没醒"。
    /// 为什么这张表长什么样、为什么它不待在结构体里 = [`super::Failures`] 头注。
    failures: Arc<Failures>,
}

impl Compactor {
    pub(crate) fn new(
        settings: Arc<SettingsStore>,
        aux: Arc<dyn MemoryManagerClient>,
        failures: Arc<Failures>,
    ) -> Self {
        Self {
            settings,
            aux,
            failures,
        }
    }

    /// 占用读数（"到线了没"的分子）。
    ///
    /// 与进度条、整理触发线同一把尺 = [`crate::context_budget::anchor`]（优先供应商
    /// 上报的 `LlmUsage`）。**唯一多出来的一条纪律是新鲜度**：若最后一次 `LlmUsage`
    /// 落在最后一次 `ContextCompacted` **之前**，那个数量的是压缩之前的窗口，照它
    /// 判定会得出"仍然超线"⇒ 本轮每个 step 再压一次、每次一条模型往返。那一档改吃
    /// 压后投影的估算——用的仍是 anchor 自己"无上报"那一档的同一把估算尺，
    /// 不是为压缩另造第三把尺。
    pub(crate) fn used(session: &Session) -> u64 {
        let stale = match (
            session.events().iter().rposition(is_usage),
            session.events().iter().rposition(is_compacted),
        ) {
            (Some(u), Some(c)) => u < c,
            _ => false,
        };
        if stale {
            estimate_tokens(&session.derive_messages())
        } else {
            crate::context_budget::anchor(session).0
        }
    }

    /// step 边界那一句。`policy` 带的是**组装点已经算好的有效阈值**（刀① 把两个
    /// 组装点的 `UsagePolicy.threshold_tokens` 都改吃 `effective_threshold`，主层
    /// 不许在这儿再算第二遍）与窗口读数。
    pub(crate) async fn run(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &UsagePolicy,
    ) -> Outcome {
        let cfg = self.settings.compact_config();
        if !cfg.enabled {
            return Outcome::Off;
        }
        // 裁定 A 在这一层的同一条姿态：窗口未知 ⇒ 不压。`window_tokens == 0` 是
        // 组装点"取不到窗口"的表示（`commands/chat.rs` 那句 `window.unwrap_or(0)`），
        // 而有效阈值在这一档原样吃滑杆——所以**不能**拿 `threshold_tokens > 0` 当
        // "窗口已知"的证据，必须单独看这一枚。
        let threshold = (policy.window_tokens > 0).then_some(policy.threshold_tokens);
        let messages = session.derive_messages();
        let used = Self::used(session);
        let plan = plan(&PlanInput {
            messages: &messages,
            threshold,
            used,
            target_pct: cfg.target_pct,
            keep_tokens: cfg.keep_tokens,
            failures: self.failures.of(session_id),
        });
        if plan.reason != "planned" {
            tracing::debug!(
                session = session_id,
                reason = plan.reason,
                used,
                threshold = policy.threshold_tokens,
                window = policy.window_tokens,
                "深度压缩本轮不动"
            );
            return Outcome::Skipped(plan.reason);
        }
        let system = build_prompt(cfg.instructions.as_deref());
        // overflow 递减带（刀④d）：第一趟喂全量，失败且**确实判成超窗**时按
        // [`super::SHRINK_STEPS`] 逐档缩到较新的一截再试。超窗在这儿不是理论风险：
        // `context.compactModel` 换的是**另一档模型**，它的窗口可以比主会话小得多
        // （阈值按主模型的窗口算，摘要请求按摘要模型的窗口撞线）。
        // 一次压缩 = 一个失败计数，不管内部试了几档：熔断管的是"这一层连着几轮没
        // 压成"，不是"共发了几条往返"（把 4 趟算成 4 次失败 = 一轮就把这一层锁死）。
        let mut history = render_history(&messages[..plan.cut]);
        let mut attempts = 0u32;
        let raw = loop {
            attempts += 1;
            match self
                .aux
                .complete(&system, &history, cfg.model.as_deref())
                .await
            {
                Ok((text, _)) => break text,
                Err(e) => {
                    // 判别只许用 `llm::retry` 那一份（不许压缩层自备关键词表）：
                    // 认错了档，普通失败会被反复缩减重试白花几条往返，
                    // 而超窗被认成普通失败 = 唯一能自愈的那一档形同不存在。
                    let overflow = crate::llm::is_overflow_message(&e);
                    let ratio = overflow
                        .then(|| SHRINK_STEPS.get(attempts as usize - 1).copied())
                        .flatten();
                    let Some(ratio) = ratio else {
                        let failures = self.failures.record(session_id);
                        tracing::warn!(
                            session = session_id,
                            failures,
                            attempts,
                            overflow,
                            error = %e,
                            "深度压缩：摘要调用失败，不落账"
                        );
                        return Outcome::Failed {
                            why: if overflow { "overflow" } else { "summary-call" },
                            failures,
                            attempts,
                        };
                    };
                    tracing::warn!(
                        session = session_id,
                        attempts,
                        ratio,
                        error = %e,
                        "摘要请求超窗 ⇒ 缩到较新的一截再试"
                    );
                    history = shrink_history(&history, ratio);
                }
            }
        };
        // 验货在落账之前，而且验的是**要落进日志的那份文本**：先归一化（剥整段围栏、
        // 收空行）再逐节点名。顺序反过来会漏一档真形状——模型常把整份摘要包进代码块，
        // 原始串里带 `##` 而剥完什么也不剩，先验原始串就"合格"了，落账的却是一份空的。
        let summary = format_summary(&raw);
        let missing = missing_sections(&summary);
        if !missing.is_empty() {
            let failures = self.failures.record(session_id);
            tracing::warn!(
                session = session_id,
                failures,
                attempts,
                missing = ?missing,
                "深度压缩：摘要不合格，不落账"
            );
            return Outcome::Failed {
                why: "rejected",
                failures,
                attempts,
            };
        }
        let kept = &messages[plan.cut..];
        let kept_messages = kept.len();
        let from_tokens = estimate_tokens(&messages);
        // 压后的实际窗口 = 摘要那一条 + 保留的近期原文。摘要消息的文本形状由
        // `session_store::summarized_message` 决定（与折叠路径同一个构造函数 =
        // 同一出处），所以这里算出来的数与下一步内核投影出来的窗口同形。
        let mut after = Vec::with_capacity(kept_messages + 1);
        after.push(summarized_message(&summary));
        after.extend_from_slice(kept);
        let to_tokens = estimate_tokens(&after);
        session.append(SessionEvent::ContextCompacted {
            kept_messages,
            summary,
            from_tokens,
            to_tokens,
            strategy: STRATEGY.to_string(),
            ts: crate::timeutil::now_ms(),
        });
        self.failures.clear(session_id);
        tracing::info!(
            session = session_id,
            from_tokens,
            to_tokens,
            kept_messages,
            strategy = STRATEGY,
            "深度压缩落账"
        );
        Outcome::Compacted {
            kept_messages,
            from_tokens,
            to_tokens,
        }
    }
}

fn is_usage(e: &SessionEvent) -> bool {
    matches!(e, SessionEvent::LlmUsage { .. })
}

fn is_compacted(e: &SessionEvent) -> bool {
    matches!(e, SessionEvent::ContextCompacted { .. })
}
