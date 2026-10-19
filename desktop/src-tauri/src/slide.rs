//! 滑窗保护网（specs/09 刀⑤ · AC-S2-8）：主层熔断之后，把**旧轮的过程**从要发出去的
//! 那份消息列里整段摘掉，只留每轮的提问与最近一截过程——它不生成摘要（生成摘要的那条
//! 通路刚刚连败三次），它只保证"这一轮请求发得出去"。
//!
//! 三层机械前置各挡一件事，互不重叠：`spill` 挡**一条**工具输出多大、`microcompact`
//! 把**很旧的**工具结果清成一行占位（消息骨架还在）、`slide` 丢**整段旧过程**（连骨架
//! 一起不发）。前两层是减速，这一层是保护网：它只在主层熔断时醒，主层某一次压成就
//! 自动退场。
//!
//! ## 为什么挂 REQUEST 环上，而不是跟主层一样挂 step 边界
//!
//! 因为形状不允许。`CompactionOutcome{kept_messages, summary}` 与它落的
//! `ContextCompacted` 只能表达"截掉头部 + 插一条摘要"（折叠实现 =
//! `session-store::compact_messages`），而"每轮留提问、过程丢掉"是**散布式**的丢弃，
//! 头部截断表达不了它。环上监听器直接改 payload 里的消息列，任意形状都能表达——
//! 微压缩同因同位（它也是散布式改写）。
//!
//! [`crate::compact`] 头注那三条"不许挂环上"的理由，对这一层一条都不成立：它不调模型
//! （重试烧 N 份 = 不适用）、它不落账（失败丢账 = 不适用）、它不需要锚点那把尺
//! （它吃的就是自己即将发出去的那份消息列的估算，与微压缩同一把尺）。
//!
//! ## 为什么不落 `ContextCompacted`
//!
//! 微压缩落账而账能重放：它的判据全在日志与时间里（哪些结果旧了、距上次收束多久），
//! 重启后重放同一份日志会清掉同一批 ⇒ 投影稳定。这一层的开关是**熔断态**，而熔断表
//! 故意是进程内的（[`crate::compact::Failures`] 头注：跨重启没有意义）。落一条
//! `strategy:"slide"` 的事件，等于在日志里记一笔"当时丢了这些"而重放时**再也算不出
//! 当时熔没熔断**——那是一份无法重现的假账，比没有账更糟（界面上会显示一层动过、
//! 而实际窗口里没有它的痕迹）。
//!
//! 所以这一层是**每轮现算的无状态动作**：日志一行不动，投影一行不动，只有发出去的那份
//! 请求变短。撤销天然——主层某次压成 ⇒ 计数清零 ⇒ 下一轮过程自动回来。观测面 =
//! `tracing::info!` 那一条读数（specs/09 非目标段：压缩的界面读数走开发者面板/诊断）。
//!
//! ## 参照里读实到的反面（2026-10-19 逐行核对本地源码）
//!
//! **两家都没有这一层**，所以"丢过程留提问"与下面那颗 `KEEP_RECENT_RUNS` 都是**我们
//! 自己的裁定**，不是照抄谁（与刀④ 那颗"目标比 50%"同性质）：
//! · ZCode `core/src/compact/policy.ts:138` —— 熔断（`reason: "circuit_breaker"`）之后
//!   `shouldCompact = false`，直接不再压，等用户手动 `/compact`；
//! · OpenCode `packages/core/src/session/compaction.ts:209` —— 注释原话
//!   "Only the user compacts when automatic compaction is off, overflow included"。
//! 也就是说两家在"摘要模型坏了"这一格上的答案都是**把问题交还给用户**。我们不做交还
//! （AC-S2-9 ② 明令全程不许出现"必须重开会话 / 手动压缩才能继续"的状态），所以才需要
//! 这层参照里没有的东西。
//!
//! ## 不许出事的那一档：assistant 与它的工具回执不能拆对
//!
//! 供应商硬约束（G2 悬空闭合那条纪律的同一条）：一条 `tool` 消息前面必须有声明它的
//! `assistant.tool_calls`。丢弃按"连续过程段"整段进行，段内天然成对；但**孤儿回执**
//! （恢复自旧日志的残段、或 user 消息插在 assistant 与回执之间）会让"丢 assistant、
//! 留回执"成为可能 ⇒ [`plan`] 里那一步闭合修正专门处理它，用例
//! `a_dropped_assistant_takes_its_own_results_out_of_the_window` 钉住。

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{estimate_tokens, points, take_request};
use serde_json::Value;
use session_store::Message;

use crate::compact::Failures;
use crate::microcompact::MainThreshold;
use crate::settings::SettingsStore;

/// 保留最近多少个**过程段**（一段 = 两条提问之间连续出现的 assistant / 工具回执）。
///
/// 1 是我们拍的：这层的目的是"发得出去"，不是"保住工作记忆"；熔断时模型已经拿不到
/// 摘要，最近一段过程是它唯一知道自己刚做过什么的机会——再少就没有了。它不做成设置项
/// 是**刀⑤ 的欠账**，登记在 specs/09 挂账 ⓕ（可配置原则①：不确定的数值要能调）。
pub(crate) const KEEP_RECENT_RUNS: usize = 1;

/// 一次判定要吃的所有读数（**纯函数**，监听器只负责把它们凑齐）。
pub(crate) struct SlideInput<'a> {
    /// 即将发出去的那份消息列（微压缩已经跑过，所以这里看到的是压后形状）。
    pub messages: &'a [Message],
    /// 有效阈值（与微压缩、主层同一把尺 = `context_budget::effective_threshold`）。
    /// `None` = 窗口未知 ⇒ 这一层不动（裁定 A 在保护网上的同一条姿态）。
    pub main_threshold: Option<u64>,
    /// 主层此刻熔断了没（`Failures::fused` 的读数）。**false ⇒ 零动作**是 AC-S2-8
    /// 要单独断言的那一档：保护网不许在正常会话里悄悄吃掉过程。
    pub fused: bool,
}

/// 判定结果：`dropped` = 要从消息列里摘掉的**下标**（升序）。
#[derive(Debug)]
pub(crate) struct SlidePlan {
    pub reason: &'static str,
    pub dropped: Vec<usize>,
    pub from_tokens: u64,
    pub to_tokens: u64,
}

impl SlidePlan {
    fn stop(reason: &'static str, from_tokens: u64) -> SlidePlan {
        SlidePlan {
            reason,
            dropped: Vec::new(),
            from_tokens,
            to_tokens: from_tokens,
        }
    }
}

/// 纯判定：要不要丢、丢哪几条。任何一步不满足都如实带回 `reason`（观测面读它）。
pub(crate) fn plan(input: &SlideInput<'_>) -> SlidePlan {
    let msgs = input.messages;
    let from_tokens = estimate_tokens(msgs);

    // 熔断没开 ⇒ 一个字都不动。这一条必须排在窗口与用量之前：正常会话里即使窗口未知、
    // 即使用量很高，保护网也不许在场（AC-S2-8 第二句）。
    if !input.fused {
        return SlidePlan::stop("not-fused", from_tokens);
    }
    let Some(threshold) = input.main_threshold else {
        return SlidePlan::stop("unknown-window", from_tokens);
    };
    // 到线才动。注意这里吃的是**微压缩之后**的占用：三层是串联的，前两层已经把量
    // 压下去过，保护网只在该它的时候该。
    if from_tokens < threshold {
        return SlidePlan::stop("below-line", from_tokens);
    }

    let runs = process_runs(msgs);
    // 段数不够 ⇒ 没有"旧的"可丢（全是要保留的最近一段）。
    let drop_count = runs.len().saturating_sub(KEEP_RECENT_RUNS.max(1));
    if drop_count == 0 {
        return SlidePlan::stop("no-process", from_tokens);
    }

    // 提问一律保留（`is_question` 那条注释说清了为什么摘要行也在这一半里）。
    let mut keep = vec![true; msgs.len()];
    for &(start, end) in &runs[..drop_count] {
        for slot in &mut keep[start..end] {
            *slot = false;
        }
    }
    // 闭合修正：被丢的 assistant 如果还有回执留在保留集里，把那条 assistant 拉回来。
    // 宁可少丢一点，也不能发出一份"工具回执没有前置声明"的请求——那种形状被供应商
    // 整单拒收，保护网反而把会话弄死。
    for (i, m) in msgs.iter().enumerate() {
        if !keep[i] {
            continue;
        }
        let Message::ToolResult { call_id, .. } = m else {
            continue;
        };
        for (j, other) in msgs.iter().enumerate() {
            if keep[j] || j > i {
                continue;
            }
            let Message::Assistant { tool_calls, .. } = other else {
                continue;
            };
            if tool_calls.iter().any(|c| &c.id == call_id) {
                keep[j] = true;
            }
        }
    }

    let dropped: Vec<usize> = (0..msgs.len()).filter(|&i| !keep[i]).collect();
    if dropped.is_empty() {
        return SlidePlan::stop("no-process", from_tokens);
    }
    // 保留后的占用**用同一把 `estimate_tokens` 量**（不按下标逐条加——那是第二把尺，
    // 与微压缩"省多少"的算法必须同形）。
    let kept: Vec<Message> = msgs
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, m)| m.clone())
        .collect();
    let to_tokens = estimate_tokens(&kept);
    // 零收益不动作（与微压缩"省不够就整批还原"同一姿态）：丢不出字节就说明这段
    // 本来就是空的，白破一次消息形状不值。
    if to_tokens >= from_tokens {
        return SlidePlan::stop("no-savings", from_tokens);
    }
    SlidePlan {
        reason: "planned",
        dropped,
        from_tokens,
        to_tokens,
    }
}

/// 一条消息是不是"提问"（保留的那一半）。
///
/// 摘要行（`session_store::summarized_message`）与整理段落行（`ContextSummary`）在投影里
/// 都是 `Message::User`，所以这一条同时保证了**压过的历史不会被这层再丢一次**——那是
/// 前史唯一的在场形式，丢了就真没了。这是故意的，不是漏判。
fn is_question(m: &Message) -> bool {
    matches!(m, Message::User { .. })
}

/// 把消息列切成"连续过程段"：`(start, end)` 左闭右开，提问消息不属于任何段。
fn process_runs(messages: &[Message]) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for (i, m) in messages.iter().enumerate() {
        if is_question(m) {
            if let Some(s) = start.take() {
                runs.push((s, i));
            }
        } else {
            start.get_or_insert(i);
        }
    }
    if let Some(s) = start {
        runs.push((s, messages.len()));
    }
    runs
}

/// 环上那半：把 [`plan`] 的判定变成对 payload 消息列的实际摘除。
pub(crate) struct Slide {
    settings: Arc<SettingsStore>,
    /// 熔断表（**与主层同一份**，由组装点注入）。两张表 = 主层已熔断而保护网没醒。
    failures: Arc<Failures>,
    /// 有效阈值取数闭包，与微压缩共用同一个类型与同一个算式出处。
    main_threshold: MainThreshold,
}

impl Slide {
    pub(crate) fn new(
        settings: Arc<SettingsStore>,
        failures: Arc<Failures>,
        main_threshold: MainThreshold,
    ) -> Self {
        Self {
            settings,
            failures,
            main_threshold,
        }
    }
}

#[async_trait]
impl WaterfallListener for Slide {
    async fn handle(&self, key: &str, payload: &mut Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        // 压缩总开关关 = 整条压缩面不参与（AC-S2-10 那句"关 = 完全不压、接近窗口只提醒"
        // 把这一层一起算在内：保护网也是"压"）。
        if !self.settings.compact_config().enabled {
            return WaterfallDecision::Continue;
        }
        let mut guard = match take_request(payload, "Slide") {
            Ok(g) => g,
            Err(d) => return d,
        };
        let req = guard.req();
        let sid = req.session_id.as_deref();
        // 没有会话身份 ⇒ 没有熔断记录可读 ⇒ 当作没熔断（宁可少做一层，不做没依据的）。
        let fused = sid.is_some_and(|s| self.failures.fused(s));
        let p = plan(&SlideInput {
            messages: &req.messages,
            main_threshold: (self.main_threshold)(sid),
            fused,
        });
        if p.reason != "planned" {
            tracing::debug!(reason = p.reason, from = p.from_tokens, "滑窗保护网未介入");
            return WaterfallDecision::Continue;
        }
        let dropped: HashSet<usize> = p.dropped.iter().copied().collect();
        let dropped_msgs = dropped.len();
        req.messages = req
            .messages
            .drain(..)
            .enumerate()
            .filter(|(i, _)| !dropped.contains(i))
            .map(|(_, m)| m)
            .collect();
        tracing::info!(
            dropped = dropped_msgs,
            kept = req.messages.len(),
            from = p.from_tokens,
            to = p.to_tokens,
            "滑窗保护网：旧过程段不发出（提问全留）"
        );
        WaterfallDecision::Continue
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ring;

/// 测试夹具：`n` 轮完整往返 = 一条提问 + 一次调用 + 它的结果。
///
/// 为什么住在被测模块里而不是两个测试文件各抄一份 = [`compact`](super) 的
/// `sample_summary` 同一条理由：这份夹具的形状（提问在最前、assistant 带调用、
/// 回执紧随）直接决定 [`process_runs`] 能切出几段，两处各写一份的话，一处改了
/// 另一处会红在"段数不对"上，而那条红看起来像被测对象坏了。
#[cfg(test)]
pub(crate) fn rounds(n: usize, size: usize) -> Vec<Message> {
    let mut out = Vec::new();
    for i in 0..n {
        out.push(Message::user(format!("第{i}轮问题")));
        let id = plugin_api::CallId(format!("c{i}"));
        out.push(Message::Assistant {
            content: format!("第{i}轮思考"),
            tool_calls: vec![session_store::ToolCallMsg {
                id: id.clone(),
                tool: "fs_read".into(),
                args: serde_json::json!({"path": format!("/f{i}.txt")}),
            }],
        });
        out.push(Message::ToolResult {
            call_id: id,
            tool: "fs_read".into(),
            args: serde_json::json!({}),
            output: serde_json::json!("x".repeat(size)),
            error: None,
        });
    }
    out
}

/// 有效阈值 = 当前估算 ⇒ 必在线内（这一层吃的是**当前**占用，不像微压缩还要再乘 0.9）。
#[cfg(test)]
pub(crate) fn over_line(messages: &[Message]) -> Option<u64> {
    Some(estimate_tokens(messages))
}

/// 阈值抬到当前估算的两倍 ⇒ 确实没到线。
#[cfg(test)]
pub(crate) fn under_line(messages: &[Message]) -> Option<u64> {
    Some(estimate_tokens(messages) * 2)
}
