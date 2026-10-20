//! 上下文预算的中立尺（specs/09 **刀①** · S2 压缩面地基）：占用锚点 + 有效阈值算式。
//!
//! 为什么存在 = **依赖方向**，不是顺手重构。这两块地原来长在 `organize/`（记忆树
//! 整理）里，而 organize 按 specs/01 阶段表 D 节 **S4 整面退役**。压缩触发线要吃
//! 锚点，锚点吃 organize ⇒ 不搬家就是"把 S2 的形状押在一个要删的模块上"。搬完之后
//! organize 继续吃**这同一份**（它不许留第二把尺，机器面 = 判据 SS16b）。
//!
//! **刀① 当时故意保持的两处不一致，现在只剩一处**（搬家不许改读数，所以那时登记不修）：
//!
//! * **回落口径原先分两种**：本模块 [`anchor`] 在无供应商上报时回落**全量历史字符估算**，
//!   而进度条（`runner::record_usage`）对树会话回落**组装后口径**（目录 + 尾巴）。
//!   ⇒ **S3 刀④ 收口**：分子统一成 [`occupancy`] 一个函数，两侧同吃全量估算，
//!   进度条那条树分支连同 `run_turn` 的 `tree` 参数一起摘掉。v2.138 那句
//!   「进度条 = 触发 = 自动收起一把尺」至此**三档都成立**（原先只管"有上报那一档"）。
//! * [`AnchorSource`] 的落点本来就是本模块：S4 刀③a 已从 `memory-tree` 迁入
//!   （树整面退役，压缩线不再反向依赖要删的 crate）。

use session_store::{Session, SessionEvent};

/// 占用锚点来源（观测面如实标注）：S4 刀③a 从 `memory-tree` 的 `trigger`
/// 迁来——树整面退役，而这枚枚举是压缩触发与进度条共用的观测标签，留下。
/// 只用 Debug/比较，序列化归界面投影（`organize/debug.rs` 自己转字符串）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnchorSource {
    /// provider 上报的真实 input tokens（会话日志最后一条 LlmUsage）。
    LlmUsage,
    /// 字符估算兜底（无 LLM 用量上报时的回落）。
    Estimate,
}

/// 输出侧预留的**缺省值**：供应商目录与档位都没告诉我们输出上限时按这颗算
/// （ZCode `compact/policy.ts:9` `DEFAULT_AUTOCOMPACT_OUTPUT_RESERVE_TOKENS`
/// 逐字 = 32_000，注释同一条理由：正常请求的输出目标已收敛到 32K，压缩窗口
/// 若按另一颗常量预留，请求预算与压缩阈值就永远是两套数）。
pub const DEFAULT_OUTPUT_RESERVE_TOKENS: u64 = 32_000;

/// 输出侧预留的**上限**（21 000，S4 · specs/11 刀① 起写死）。
///
/// ZCode `compact/policy.ts:10` `PREFLIGHT_AUTOCOMPACT_OUTPUT_RESERVE_TOKENS` 逐字：
/// 自动压缩只能让出输入侧，为整颗模型输出上限让窗口会把压缩点推得过早，所以预留取
/// `min(输出上限 ?? 32K, 这一颗)`。S2/S3 期间它曾是设置项，现在回到常量——用户没有
/// 依据去判断 21K 与 30K 哪个"更优"，这一格不该让人填。
pub(crate) const OUTPUT_RESERVE_CAP_TOKENS: u64 = 21_000;

/// 输出侧预留量（token）：`min(输出上限 ?? 32K, 固定上限 21K)`。
///
/// 上限的取值 = [`OUTPUT_RESERVE_CAP_TOKENS`]（ZCode `policy.ts:10`
/// `PREFLIGHT_AUTOCOMPACT_OUTPUT_RESERVE_TOKENS` 逐字；为什么要有上限：模型的
/// `max_output` 可以大到 64K 甚至更高，而自动压缩只能让出**输入侧**——为整颗输出
/// 上限让窗口，会把压缩点推得过早，`policy.ts:77` 原话）。S3 刀④b 曾把它抬成可调项、
/// 并在浮层里展示"这颗数从哪儿来"，S4（specs/11 刀①）把两者一起撤掉：用户没有依据
/// 判断 21K 与 30K 哪个更优，读数也不再需要在界面上解释来源。
///
/// 上限已知且小于 cap ⇒ 就按它扣（那一档是 `min` 的另一半，测试
/// `reserve_takes_the_smaller_of_ceiling_and_cap` 专门钉它——只测"大值被削平"的话，
/// 把 `min` 写成常量也测不出来）。
fn output_reserve(max_output: Option<u64>, cap: u64) -> u64 {
    match max_output {
        Some(m) if m <= cap => m,
        Some(_) => cap,
        None => DEFAULT_OUTPUT_RESERVE_TOKENS.min(cap),
    }
}

/// 占用锚点（v2.138 统一锚点）：优先供应商真实 input（日志最后一条 `LlmUsage`
/// 的 prompt_tokens，含缓存命中 = 模型实际收到的上下文），无上报回落字符估算，
/// **锚点来源如实标注**。
///
/// 从 `Organizer::anchor` 整块搬来（刀①）：扫描逻辑与回落表达式**逐字未动**，
/// 等价性证据 = organize 既有测试一字未改仍绿。
pub(crate) fn anchor(session: &Session) -> (u64, AnchorSource) {
    match crate::runner::reported_input_tokens(session) {
        Some(input) => (input, AnchorSource::LlmUsage),
        None => (
            kernel_agent::estimate_tokens(&session.derive_messages()),
            AnchorSource::Estimate,
        ),
    }
}

fn is_usage(e: &SessionEvent) -> bool {
    matches!(e, SessionEvent::LlmUsage { .. })
}

fn is_compacted(e: &SessionEvent) -> bool {
    matches!(e, SessionEvent::ContextCompacted { .. })
}

/// 占用分子（**唯一算法** = S3 刀④ 要的那"一个出处"，账 ⓟ）：三档取最大。
///
/// 1. **供应商上报**（新鲜时）—— 走 [`anchor`]，与整理触发线、自动收起同尺
///    （v2.138 那句「看到的 = 触发用的」）。
/// 2. **上报过期时的全量字符估算** —— 最后一次 `LlmUsage` 落在最后一次
///    `ContextCompacted` **之前** ⇒ 那个数量的是压缩**之前**的窗口，照它判定会得出
///    "仍然超线"，于是本轮每个 step 再压一次、每次多一条模型往返。
/// 3. **被超窗拒收那颗数**（S2 刀⑧ · [`crate::compact::Overflow]）—— 被拒的那一次
///    既没有上报也没有压缩事件，上面两式都追不到它 ⇒ 触发线永远停在"被拒之前"
///    （实测形状：连续 41 个失败轮、两层各 0 条落账）。
///
/// 三档取**最大**而不是"换尺"：窗口在被压小之前只会变大，任何一档读数都是下界。
///
/// ⚠ `refused` 由**调用方**从同一张 `Overflow` 表取（组装点与压缩面持的是同一个
/// `Arc`，为什么必须是同一个 = [`crate::compact::Failures`] 头注那条同款纪律）。
/// 本函数故意不吃表：给定（日志, refused）读数唯一 ⇒ 它能被纯函数判据钉住，
/// 两个调用点若各自再补一次 `max`，判据就抓不到了。
///
/// **本刀统一掉的第二把尺**（S2 刀① 登记在头注、当时明写"留给刀④"）：进度条原先在
/// 无上报那一档对**树会话**用"组装后口径"（目录 + 尾巴，M4 §3.38②），压缩面用全量
/// 历史估算 ⇒ 同一轮两处不同数。这里把进度条那侧的树分支摘掉、两侧同吃全量估算。
/// **这是读数口径的变更，不是修 bug**：树会话在无上报那一档进度条会**变大**（更保守）。
/// 方向为什么对：S3 刀③a 之后树目录不再进上下文，"目录 + 尾巴"量的已经不是模型收到的
/// 那份东西，而全量历史是它的上界。
pub(crate) fn occupancy(session: &Session, refused: u64) -> u64 {
    let stale = match (
        session.events().iter().rposition(is_usage),
        session.events().iter().rposition(is_compacted),
    ) {
        (Some(u), Some(c)) => u < c,
        _ => false,
    };
    let base = if stale {
        kernel_agent::estimate_tokens(&session.derive_messages())
    } else {
        anchor(session).0
    };
    base.max(refused)
}

/// 有效压缩阈值 = `min(滑杆值, 窗口 − 输出侧预留)`，**输出侧只扣一次**。
///
/// 为什么左边要 `min` 滑杆：滑杆（`context.thresholdTokens`）是用户意图，窗口封顶
/// 是物理约束，取小 = "用户说不超过 150K 就绝不超，模型只给 179K 空位时也绝不撞线"。
/// 为什么不许再减第二遍（AC-S2-2 的靶）：`窗口 − 预留 − 预留` 是 ZCode 订正前的
/// legacy 形状，实测会表现为"进度条离满还差一截就压缩"，而用户看到的分母只有一个。
///
/// **窗口取不到（`None`）那一档的裁定**（方案稿支撑层实证③要求"写死一个，别留给
/// 实现临场决定"；2026-10-19 用户采纳建议 = 保守档）：不做任何窗口假定，阈值原样
/// 吃滑杆。另一半裁定——"因此不触发自动压缩"——**不在本刀落地**，因为现网还没有
/// 自动压缩的执行者；它归刀④ 的主层触发线（`specs/09` 刀④ 那条），在这里预造一个
/// `armed()` 判据就是取证①点名的"存在但从未接线"那个形状。
///
/// S4（specs/11 刀①）：第四枚 `cap` 由调用方传
/// [`OUTPUT_RESERVE_CAP_TOKENS`]（上限已写死）；**这一式仍是唯一真源**
/// （判据 SS16c 第三档），浮层不再展示扣减链。
pub(crate) fn effective_threshold(
    slider: u64,
    window: Option<u64>,
    max_output: Option<u64>,
    cap: u64,
) -> u64 {
    match window {
        None => slider,
        Some(w) => slider.min(w.saturating_sub(output_reserve(max_output, cap))),
    }
}

#[cfg(test)]
mod tests;
