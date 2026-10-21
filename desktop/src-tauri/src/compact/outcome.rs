//! 这一趟的结果与**失败判据名**（specs/13 契约 C3）。
//!
//! 判据名是**后端 → 前端**的跨语言契约：后端只给名字，前端按名字查语言包。
//! 所以它必须住在**一个可被机械扫描的地方**（同源闸 SS24 扫的就是 [`REASONS`]
//! 那一行），且**字面量只许出现那一次**——别处引用常量。
//!
//! **跳过 vs 失败**：跳过（没到线 / 开关关着 / 刚压完）**不产事件**、
//! **不进界面文案表** ⇒ 它们不是判据名，只是内部状态（[`Skip`]）。
//! 熔断也不在表里：它走相位 `paused`（上游同款相位分离）。

/// 失败判据名（specs/13 契约 C3）。**唯一出处**——界面那张文案表按同名对账
/// （同源闸 SS24 扫的就是这一行）。
pub(crate) const REASONS: [&str; 9] = [
    "nothing-to-compact",
    "template-mismatch",
    "no-summary",
    "output-limit",
    "cannot-shrink",
    "provider-error",
    "content-filter",
    "interrupted",
    "budget",
];

// 判据名的**具名引用**（字面量只住在 [`REASONS`] 一处）。
//
// 为什么不用 `pub(crate) const X: &str = "..."`：那样每个名字都有两处出处，
// 改一处忘另一处 = 界面查表落进兜底。索引取自 [`REASONS`] 的顺序，
// 下面对账测试钉住"常量 == 表里那一项"。
pub(crate) const NOTHING_TO_COMPACT: &str = REASONS[0];
pub(crate) const TEMPLATE_MISMATCH: &str = REASONS[1];
pub(crate) const NO_SUMMARY: &str = REASONS[2];
pub(crate) const OUTPUT_LIMIT: &str = REASONS[3];
pub(crate) const CANNOT_SHRINK: &str = REASONS[4];
pub(crate) const PROVIDER_ERROR: &str = REASONS[5];
pub(crate) const CONTENT_FILTER: &str = REASONS[6];
pub(crate) const INTERRUPTED: &str = REASONS[7];
pub(crate) const BUDGET: &str = REASONS[8];

/// 跳过（**不产事件**、不进界面文案表）。
///
/// 为什么单独一档而不是塞进判据名：跳过是常态（不到线 / 开关关着 / 刚压完），
/// 每一步都推一句「没压成」就是把界面吵醒；而判据名那一族是**要给人看的**
/// 「为什么没成」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skip {
    /// 占用没到门槛（自动档）。
    BelowLine,
    /// 自动压缩开关关着（自动档）。
    Disabled,
    /// 刚压完（上游 `due()` 的第一条守卫：上一条就是一次压完的压缩 ⇒ 不压）。
    JustCompacted,
}

/// 这一趟的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// 这一趟没动（**不产事件**）。
    Skipped(Skip),
    /// **熔断停手**（连败到线）⇒ 相位 `paused`。手动那一格让开它（人在看着、
    /// 要它再试），所以手动侧理论不可达——但变体得穷尽，编译器盯着。
    Paused,
    /// 压成了。
    Compacted {
        /// 保留区的条数（保留指针所指那一段）。
        kept_messages: usize,
        /// 压前的估算占用。
        from_tokens: u64,
        /// 压后的估算占用。
        to_tokens: u64,
    },
    /// 没成：`reason` = 判据名（[`REASONS`] 之一），`detail` = 细节文本
    /// （供应商原文 / 上游那句说明）。**界面读 `reason`**（按名查文案），
    /// `detail` 只进日志与事件载荷的 `text` 那一格。
    Failed {
        reason: &'static str,
        detail: Option<String>,
    },
}

impl Outcome {
    /// 这一趟的判据名（失败才有；跳过与压成都没有）。
    pub(crate) fn reason(&self) -> Option<&'static str> {
        match self {
            Outcome::Failed { reason, .. } => Some(reason),
            _ => None,
        }
    }
}

/// 上游那两句失败说明（逐字）。
///
/// 为什么它们住在后端而不是语言包：它们是**上游契约的一部分**（上游
/// `compaction.ts:311-313` 就是这两句），判据/用例要逐字对账；而界面上给人看的
/// 那句话由前端按判据名查语言包 ⇒ 这两句不进界面文案表。
pub(crate) const MSG_TEMPLATE_MISMATCH: &str =
    "Compaction summary did not match the required template";
pub(crate) const MSG_NO_SUMMARY: &str = "Compaction produced no summary";
/// 上游「再也缩不动了」那一句（`compaction.ts:437-439` 逐字）。
pub(crate) const MSG_CANNOT_SHRINK: &str =
    "The compaction request cannot be reduced further without losing the latest exchange or checkpoint";
/// 上游「撞输出上限」那一句（`compaction.ts:572-575` 逐字）。
pub(crate) const MSG_OUTPUT_LIMIT: &str = "Compaction summary reached the output token limit";
/// 上游「被内容过滤」那一句（`compaction.ts:577` 逐字）。
pub(crate) const MSG_CONTENT_FILTER: &str = "Compaction summary was blocked by the provider";
/// 上游「没有可摘的」那一句（`compaction.ts:91` 逐字）。
pub(crate) const MSG_NOTHING_TO_COMPACT: &str = "Nothing to compact yet";
/// 上游「被叫停」那一句（`compaction.ts:685` 逐字）。
pub(crate) const MSG_INTERRUPTED: &str = "Compaction was interrupted";
