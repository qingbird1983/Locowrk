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
//! 不换尺就会每个 step 再烧一条摘要往返；刀⑧ 又补了第三档：**被供应商按超窗拒收的
//! 那一次压根没有 `LlmUsage`**（它在处理之前就吃了闭门羹），不另记这一笔，触发线就
//! 永远读不到"已经超了"，这一层当场自锁（台账 ⓜ，读数与形状见 [`super::Overflow`]）；
//! ② **先验货再落账**——验的是归一化之后那一份文本（模型把整份摘要包进代码块、
//! 剥完什么都不剩，也算不合格），不合格只进失败计数（熔断的输入），不进日志；
//! ③ `to_tokens` 用**实测**（摘要文本此刻才存在），与判定时的目标位分属"预估"与
//! "落账实测"，不是同一读数的两个出处；
//! ④ **overflow 递减只认超窗**（刀④d）：判别用 `llm::retry` 那一份，一次压缩无论
//! 试了几档都只算一个失败计数——把 4 趟算成 4 次失败，等于一轮就把这一层锁死。

use std::sync::Arc;
use std::time::Duration;

use kernel_agent::estimate_tokens;
use session_store::{summarized_message, Session, SessionEvent};

use super::{
    plan,
    prompt::{build_prompt, format_summary, missing_sections, render_history, shrink_history},
    Failures, Overflow, PlanInput, SHRINK_STEPS,
};
use crate::llm::SummaryClient;
use crate::runner::UsagePolicy;
use crate::settings::SettingsStore;

/// `ContextCompacted.strategy` 在这一层的取值（唯一出处：写侧与判据都读它，
/// 不许在第二处敲同一个字面量）。
pub(crate) const STRATEGY: &str = "compact";

/// 失败缘由的三枚名字——**唯一出处**，与 [`STRATEGY`] 同一条纪律。
///
/// 为什么它们要当常量而不是就地敲字符串：刀⑦ 的 `/压缩` 把"这一趟为什么没动"
/// **原样交给界面**（后端不拼人话，文案住语言包），于是这三枚串成了跨语言契约：
/// 改名字而前端 `COMPACT_SKIP_KEYS` 没跟着改，界面上那句准确说明会掉进兜底、
/// 变成 `没压缩：overflow` 这种半吊子。判据 `SS24` 钉两侧相等。
pub(crate) const WHY_CALL: &str = "summary-call";
pub(crate) const WHY_OVERFLOW: &str = "overflow";
pub(crate) const WHY_REJECTED: &str = "rejected";
/// 预算耗尽（specs/12 刀①）：第四枚名字——「摘要调用超出了总时长预算」与
/// 「摘要模型没答上」是两句不同的真话，界面必须能分开说（判据 =
/// `llm::retry::is_budget_exhausted`，唯一出处）。
pub(crate) const WHY_BUDGET: &str = "budget";
/// 用户主动停（specs/12 刀①）：**不是失败**——它走 [`Outcome::Skipped`] 而不
/// 走 [`Outcome::Failed`]，因此**不推进熔断计数**（停过三次不该让压缩永久停手）。
pub(crate) const WHY_STOPPED: &str = "stopped";

/// 一次尝试的读数。四个变体各对应"界面上该说什么"——判据不许靠"返回 0"
/// 反推是哪一步拦的（S2 整理线 `Release::Skip` 同一条纪律，那个模块已随 S4 删除）。
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
    /// 摘要走**辅助补全的直呼通路**（`llm/aux_client.rs`）：压缩不是本轮对话的
    /// 一部分，不该带工具目录、也不该吃会话级模型覆盖。S4 刀③a 起取的是主模型
    /// 端点与档位（记忆管理器模型档不再参与）。
    aux: Arc<dyn SummaryClient>,
    /// 连续失败表（熔断的输入）。**由组装点注入**而不是自己 new 一枚：
    /// 观测与测试面要读同一张表；两张表 = 两套熔断状态。
    /// 为什么这张表长什么样、为什么它不待在结构体里 = [`super::Failures`] 头注。
    failures: Arc<Failures>,
    /// 超窗拒收的读数（刀⑧ · [`super::Overflow`]）：**写它的是发送侧**，本层只读 +
    /// 压成之后清。同一条"两个使用方不各建一张"的纪律 ⇒ 由组装点注入。
    refusals: Arc<Overflow>,
    /// 停止标记（specs/12 刀①）：摘要请求在等重试与吐字两处都查它 ⇒ 用户点
    /// 「停止生成」能停掉这条用户看不见的辅助请求。None = 没有停止面
    /// （headless / 测试装配）。
    stop: Option<Arc<crate::stop::StopFlags>>,
}

impl Compactor {
    pub(crate) fn new(
        settings: Arc<SettingsStore>,
        aux: Arc<dyn SummaryClient>,
        failures: Arc<Failures>,
        refusals: Arc<Overflow>,
        stop: Option<Arc<crate::stop::StopFlags>>,
    ) -> Self {
        Self {
            settings,
            aux,
            failures,
            refusals,
            stop,
        }
    }

    /// 该会话是否已被请求停止（specs/12 刀①）。
    fn is_stopped(&self, session_id: &str) -> bool {
        self.stop.as_ref().is_some_and(|f| f.is_stopped(session_id))
    }

    /// 占用读数（"到线了没"的分子）。**S3 刀④a 起算式不在这里**——三档取最大的
    /// 完整推导（供应商上报 / 上报过期时的全量估算 / 被超窗拒收那颗数，以及
    /// "为什么取最大而不是换尺"）住在 [`crate::context_budget::occupancy`]，
    /// 进度条（`runner::record_usage`）吃的是**同一个函数**。
    ///
    /// 本方法只剩两格取数：拒收读数经 [`Self::refusal`]，算式交出去。
    /// 为什么这个薄封装不许摊平成"两处各自调 occupancy"——压缩面对"到线了没"
    /// 这句问话有自己的口径（它要的是**判定用的那一次**读数），把它摘掉就等于
    /// 让 `plan` 直接吃组装点算好的数，而组装点不知道熔断/新鲜度这些层内事。
    /// S2 刀④ 起本方法**只在这一侧承重**，进度条那一侧调的是同一个 `occupancy`。
    pub(crate) fn used(&self, session: &Session, session_id: &str) -> u64 {
        crate::context_budget::occupancy(session, self.refusal(session_id))
    }

    /// 这一会话最近一次被供应商按超窗拒收时的占用读数（没有记录 = 0）。
    ///
    /// S3 刀④a 加这一格的原因就是 ⓟ：进度条的分子也要吃第 3 档，而那张表是
    /// `Compactor` 的私有件、两个组装点（聊天与定时）**本来就各持一份
    /// `Arc<Compactor>`** ⇒ 读口留在这一个 struct 上，宿主不必再多一个"谁都能碰
    /// 那张表"的字段。为什么定时那条链同样必须有它：它跑的会话一样会撞超窗，
    /// 缺这一档就是那条链的进度永久低估（同一轮里压缩已经起步、进度条还空着）。
    pub(crate) fn refusal(&self, session_id: &str) -> u64 {
        self.refusals.reading(session_id)
    }

    /// step 边界那一句 = **自动**通路（runner 到线时调）。`policy` 带的是**组装点
    /// 已经算好的有效阈值**（刀① 把两个组装点的 `UsagePolicy.threshold_tokens`
    /// 都改吃 `effective_threshold`，主层不许在这儿再算第二遍）与窗口读数。
    pub(crate) async fn run(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &UsagePolicy,
    ) -> Outcome {
        self.try_compact(session, session_id, policy, false).await
    }

    /// `/压缩` 那一句 = **手动**通路（S2 刀⑦ · AC-S2-11）。与自动通路共用同一段判定
    /// 与落账（[`Self::try_compact`] ⇒ 手动压出来的那份摘要与自动的**同形**），只差
    /// 两格门：① 不看 `cfg.enabled`——那枚开关关的是"这一层**自己**动不动手"，人主动
    /// 按的这一下不该被它挡（同设置页那句「手动 `/压缩` 不受此限」在压缩层的同一条姿态，
    /// 也是可配置原则②的反面：开关不许长成禁止）；② [`super::plan`] 带 `manual`，
    /// 让开到线与熔断两格（为什么只让这两格、三条"形状"一条不让，见那里）。
    pub(crate) async fn run_manual(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &UsagePolicy,
    ) -> Outcome {
        self.try_compact(session, session_id, policy, true).await
    }

    /// 两条通路的共用主体。`manual` 一路带到 [`super::plan`]，中间不改变量。
    async fn try_compact(
        &self,
        session: &mut Session,
        session_id: &str,
        policy: &UsagePolicy,
        manual: bool,
    ) -> Outcome {
        let cfg = self.settings.compact_config();
        if !manual && !cfg.enabled {
            return Outcome::Off;
        }
        // 裁定 A 在这一层的同一条姿态：窗口未知 ⇒ 不压。`window_tokens == 0` 是
        // 组装点"取不到窗口"的表示（`commands/chat.rs` 那句 `window.unwrap_or(0)`），
        // 而有效阈值在这一档原样吃滑杆——所以**不能**拿 `threshold_tokens > 0` 当
        // "窗口已知"的证据，必须单独看这一枚。
        let threshold = (policy.window_tokens > 0).then_some(policy.threshold_tokens);
        let messages = session.derive_messages();
        let used = self.used(session, session_id);
        let plan = plan(&PlanInput {
            messages: &messages,
            threshold,
            used,
            target_pct: super::TARGET_PCT,
            keep_tokens: super::KEEP_TOKENS,
            failures: self.failures.of(session_id),
            manual,
        });
        if plan.reason != "planned" {
            tracing::debug!(
                session = session_id,
                reason = plan.reason,
                used,
                manual,
                threshold = policy.threshold_tokens,
                window = policy.window_tokens,
                "深度压缩这一趟没动"
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
                .complete(&crate::llm::SummaryRequest {
                    system: &system,
                    user: &history,
                    model: cfg.model.as_deref(),
                    // 归属会话只为停止标记与日志归属（specs/12 刀①）：摘要不吃
                    // 会话级模型覆盖，`LlmRequest.session_id` 那一侧仍留空。
                    session_id: Some(session_id),
                    // 总时长预算（specs/12 刀①）：超了就是一次失败，推进熔断。
                    budget: Some(Duration::from_millis(cfg.budget_ms)),
                })
                .await
            {
                Ok((text, _)) => break text,
                Err(e) => {
                    // 用户主动停 ⇒ **不是失败**：记进熔断账会让"停过三次"之后
                    // 自动压缩永久停手（specs/12 刀①）。
                    if self.is_stopped(session_id) {
                        return Outcome::Skipped(WHY_STOPPED);
                    }
                    // 判别只许用 `llm::retry` 那一份（不许压缩层自备关键词表）：
                    // 认错了档，普通失败会被反复缩减重试白花几条往返，
                    // 而超窗被认成普通失败 = 唯一能自愈的那一档形同不存在。
                    let overflow = crate::llm::is_overflow_message(&e);
                    // 预算耗尽是第三档：它不是"模型没答上"，也不是"超窗"——
                    // 界面要说的话与那两句都不同（specs/12 刀①）。
                    let budget = crate::llm::is_budget_exhausted(&e);
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
                            budget,
                            error = %e,
                            "深度压缩：摘要调用失败，不落账"
                        );
                        return Outcome::Failed {
                            why: if budget {
                                WHY_BUDGET
                            } else if overflow {
                                WHY_OVERFLOW
                            } else {
                                WHY_CALL
                            },
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
        // 停止可能发生在**吐字途中**（流式回调点截断）：那时摘要回来的是半截
        // 文本 ⇒ 验货必然不过 ⇒ 不能记成「摘要不合格」。用户主动停不是失败
        // （specs/12 刀①）。
        if self.is_stopped(session_id) {
            return Outcome::Skipped(WHY_STOPPED);
        }
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
                why: WHY_REJECTED,
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
        // 拒收读数一并清掉：窗口此刻已经比被拒那一次小了，留着它 = 一次旧拒收把这层
        // 永久顶在线上方（与刀④ 那条"每个 step 重压一次"同一个坑）。
        self.refusals.clear(session_id);
        tracing::info!(
            session = session_id,
            from_tokens,
            to_tokens,
            kept_messages,
            manual,
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
