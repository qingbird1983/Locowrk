//! 上下文压缩（specs/13：按 OpenCode V2 整块重写）。
//!
//! 这一族是**主对话之外**的那条通路：把一段历史交给模型写成一份摘要，再用
//! 「摘要 + 近期原文」替换掉整段历史。它与主对话轮共用**同一条请求装配**
//! （内核 `Kernel::prepare_request`）与**同一套模型解析**（`AppLlm::prepare`），
//! 只有三处不同：总时长预算、不推 `agent://llm-retry`、delta 走自己的回调。
//!
//! 分工（按上游 `packages/core/src/session/compaction.ts` 的面切）：
//!
//! * [`prompt`]  —— 摘要提示词（上游逐字）+ 输出校验（任一标题行即通过）。
//! * [`plan`]    —— 门槛算式（窗口 × 用户百分比）+ 保留区切点（保留指针）。
//! * [`recent`]  —— 压成纯文本那一档的转录形态（丢最老 + `[N older exchange(s) omitted]`）。
//! * [`deliver`] —— 收缩矩阵（原样 → 去保留区 → 压文本丢最老 → 70%/50%/35%）。
//! * [`outcome`] —— 失败判据名（唯一出处）与这一趟的结果。
//! * [`deep`]    —— 接线半（三种触发 + 落账）。
//! * [`tables`]  —— 两张共用表（熔断账 / 超窗拒收读数）。
//!
//! **为什么判据与执行分两个面**：判定那一半是纯函数（没有 IO、没有设置、没有会话），
//! 所以"门槛怎么算、切在哪、什么算通过"能脱离桌面端跑；执行那一半才有网络与日志。

pub(crate) mod deep;
pub(crate) mod deliver;
pub(crate) mod outcome;
pub(crate) mod plan;
pub(crate) mod prompt;
pub(crate) mod recent;
pub(crate) mod tables;

/// 两张共用表的**重新导出**（真身 = [`tables`]）。
///
/// 为什么重新导出而不是让调用方改路径：调用方（组装点 / 观测面 / 测试）认的是
/// `crate::compact::Failures` 这条路径，拆文件不该逼它们改 import。
pub(crate) use tables::{Failures, Overflow};

/// 这一趟的结果与失败判据名（[`outcome`] 面）。
pub(crate) use outcome::Outcome;

/// 连续失败几次就熔断不再压（specs/13 · D2：我们自己保留的那两枚保险之一）。
///
/// 熔断**不是**失败判据名：它走相位 `paused`（上游同款相位分离）。
pub(crate) const MAX_COMPACT_FAILURES: u32 = 3;

/// 熔断线：失败计数到这颗数就断（[`MAX_COMPACT_FAILURES`] 的**唯一比较点**）。
///
/// 抽成函数而不是在调用点就地写 `>=`：比较点只有一个，将来调那颗数时
/// 不会漏掉第二处。
pub(crate) fn is_fused(failures: u32) -> bool {
    failures >= MAX_COMPACT_FAILURES
}

/// 摘要之外保留多少 token 的近期原文（上游 `compaction.ts:200` `keep: 15_000`
/// 逐字）。写死，不做设置项（同上游）。
pub(crate) const KEEP_TOKENS: u64 = 15_000;

/// overflow 应急递减带（上游 `compaction.ts:93` `SHRINK_STEPS` 逐字）。
///
/// 方向是"丢最旧"：摘要最要保住的是"最近在做什么、下一步是什么"。
pub(crate) const SHRINK_STEPS: [f64; 3] = [0.7, 0.5, 0.35];

/// 窗口未知时假定的窗口（上游 `compaction.ts:103` `UNKNOWN_WINDOW` 逐字）。
///
/// 只用于**手动 / 溢出**两档的"瞄多大"（上游同款：自动那一档窗口未知就
/// **不触发**）。自动档不拿它当窗口——那会让"窗口未知"变成"按 200K 触发"。
pub(crate) const UNKNOWN_WINDOW: u64 = 200_000;
