//! 上下文预算的中立尺（specs/09 **刀①** · S2 压缩面地基）：占用锚点 + 有效阈值算式。
//!
//! 为什么存在 = **依赖方向**，不是顺手重构。这两块地原来长在 `organize/`（记忆树
//! 整理）里，而 organize 按 specs/01 阶段表 D 节 **S4 整面退役**。压缩触发线要吃
//! 锚点，锚点吃 organize ⇒ 不搬家就是"把 S2 的形状押在一个要删的模块上"。搬完之后
//! organize 继续吃**这同一份**（它不许留第二把尺，机器面 = 判据 SS16b）。
//!
//! 两处**故意保持的不一致**（刀① 只做搬家，而搬家不许改读数，所以登记不修）：
//!
//! * 回落口径分两种：本模块 [`anchor`] 在无供应商上报时回落**全量历史字符估算**；
//!   进度条（`runner::record_usage`）回落**树组装后口径**（目录 + 尾巴）。v2.138
//!   那句「进度条 = 触发 = 自动收起一把尺」证的是**有上报那一档**（`LlmUsage`
//!   同一个数），无上报那一档两侧本来就不同数。统一它会改整理的触发读数 =
//!   改行为，那是刀④ 主层触发线接入时的决定，不在搬家这一刀偷偷做。
//! * [`memory_tree::AnchorSource`] 仍住在 memory-tree：搬家只动位置不动形状。
//!   S4 退役树时这个枚举的落点就是本模块，届时一起收。

use memory_tree::AnchorSource;
use session_store::Session;

/// 输出侧预留的**缺省值**：供应商目录与档位都没告诉我们输出上限时按这颗算
/// （ZCode `compact/policy.ts:9` `DEFAULT_AUTOCOMPACT_OUTPUT_RESERVE_TOKENS`
/// 逐字 = 32_000，注释同一条理由：正常请求的输出目标已收敛到 32K，压缩窗口
/// 若按另一颗常量预留，请求预算与压缩阈值就永远是两套数）。
pub const DEFAULT_OUTPUT_RESERVE_TOKENS: u64 = 32_000;

/// 输出侧预留的**上限**（ZCode `policy.ts:10`
/// `PREFLIGHT_AUTOCOMPACT_OUTPUT_RESERVE_TOKENS` 逐字 = 21_000）。为什么要有上限：
/// 模型的 `max_output` 可以大到 64K 甚至更高，而自动压缩只能让出**输入侧**——
/// 为整颗输出上限让窗口，会把压缩点推得过早（`policy.ts:77` 原话：legacy 分支
/// "为完整模型输出预留窗口，既过早压缩又要求远端选择"）。所以预留 = 二者取小。
pub const OUTPUT_RESERVE_CAP_TOKENS: u64 = 21_000;

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

/// 输出侧预留量（token）：`min(输出上限 ?? 32K, 21K)`。
///
/// 输出上限未知 ⇒ `min(32K, 21K)` = 21K；钉死值小于 21K ⇒ 就按它扣（那一档是
/// `min` 的另一半，测试 `reserve_follows_a_small_ceiling` 专门钉它——只测"大值被
/// 削平"的话，把 `min` 写成常量 21K 也测不出来）。
pub(crate) fn output_reserve(max_output: Option<u64>) -> u64 {
    max_output
        .unwrap_or(DEFAULT_OUTPUT_RESERVE_TOKENS)
        .min(OUTPUT_RESERVE_CAP_TOKENS)
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
pub(crate) fn effective_threshold(
    slider: u64,
    window: Option<u64>,
    max_output: Option<u64>,
) -> u64 {
    match window {
        None => slider,
        Some(w) => slider.min(w.saturating_sub(output_reserve(max_output))),
    }
}

#[cfg(test)]
mod tests;
