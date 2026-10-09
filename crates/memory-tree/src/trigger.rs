//! 触发决策（树 v2 §4.1–4.3，纯函数；v2.137 起阈值改**绝对 token**——
//! 用户裁定「达到多少 K 就触发」，不再按窗口百分比折算）：批量档 =
//! 占用 ≥ 整理阈值；增量档 = 尾巴估算 ≥ 增量块 X。占用锚点**优先
//! provider 真实 input**（LlmUsage，§4.2——与浮层估算口径的差异在 M4
//! 组装器批统一）。绝对阈值不依赖窗口，窗口未知也能触发；阈值 0 = 关
//! 自动（宁可不整，手动 `/整理` 不受此限）。没有隐藏的第二把尺（纪律 6）。

use serde::{Deserialize, Serialize};

/// 占用锚点来源（观测面板如实标注）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorSource {
    /// provider 上报的真实 input tokens（会话日志最后一条 LlmUsage）。
    LlmUsage,
    /// 字符估算兜底（无 LLM 用量上报时的回落）。
    Estimate,
}

/// 整理节奏（§4.3 二档，可随时切换）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Pacing {
    /// 批量（缺省）：达阈值一次性整理整段尾巴。
    #[default]
    Batch,
    /// 增量：尾巴每满 X token 整理一次。
    Incremental,
}

impl Pacing {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Batch => "batch",
            Self::Incremental => "incremental",
        }
    }
    /// settings.json 字符串解析（未知值回落缺省批量）。
    pub fn parse(s: &str) -> Self {
        match s {
            "incremental" => Self::Incremental,
            _ => Self::Batch,
        }
    }
}

/// 触发原因（台账 reason 字段与观测面板用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizeReason {
    Threshold,
    IncrementalChunk,
    Manual,
    Recovery,
    /// 模式 B 固定窗口的孤儿轮归档（M4 §3.38②）：窗口滑出的轮次既不在
    /// 尾巴也不在目录，必须及时归档——例行维护，**不记反弹**（§5.5 熔断
    /// 只惩罚「整理后占用立刻再超」的烧钱信号）。
    WindowOverflow,
}

impl OrganizeReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Threshold => "threshold",
            Self::IncrementalChunk => "incremental_chunk",
            Self::Manual => "manual",
            Self::Recovery => "recovery",
            Self::WindowOverflow => "window_overflow",
        }
    }
}

/// 触发判定输入（桌面端每轮收束后装配；token 单位统一）。
#[derive(Debug, Clone, Copy)]
pub struct TriggerInput {
    pub anchor_source: AnchorSource,
    pub anchor_tokens: u64,
    /// 整理阈值（绝对 token，v2.137）：占用达到即触发批量整理；0 = 关自动。
    pub threshold_tokens: u64,
    pub pacing: Pacing,
    pub chunk_tokens: u64,
    /// 未整理尾巴的估算 token（material 估算口径）。
    pub tail_tokens: u64,
    /// 未整理尾巴的内容事件数（0 = 没有可整理的东西）。
    pub tail_events: usize,
}

/// 自动触发决策：Some(原因) = 该整理；None = 不动。
pub fn should_auto_organize(inp: &TriggerInput) -> Option<OrganizeReason> {
    if inp.tail_events == 0 {
        return None;
    }
    match inp.pacing {
        Pacing::Incremental => (inp.chunk_tokens > 0 && inp.tail_tokens >= inp.chunk_tokens)
            .then_some(OrganizeReason::IncrementalChunk),
        Pacing::Batch => (inp.threshold_tokens > 0 && inp.anchor_tokens >= inp.threshold_tokens)
            .then_some(OrganizeReason::Threshold),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp(pacing: Pacing) -> TriggerInput {
        TriggerInput {
            anchor_source: AnchorSource::LlmUsage,
            anchor_tokens: 120_000,
            threshold_tokens: 100_000,
            pacing,
            chunk_tokens: 50_000,
            tail_tokens: 60_000,
            tail_events: 12,
        }
    }

    #[test]
    fn batch_fires_on_threshold() {
        assert_eq!(
            should_auto_organize(&inp(Pacing::Batch)),
            Some(OrganizeReason::Threshold)
        );
    }

    #[test]
    fn batch_below_threshold_and_empty_tail_stay_silent() {
        let mut i = inp(Pacing::Batch);
        i.anchor_tokens = 99_999;
        assert_eq!(should_auto_organize(&i), None);
        i.anchor_tokens = 120_000;
        i.tail_events = 0;
        assert_eq!(should_auto_organize(&i), None);
    }

    #[test]
    fn batch_fires_without_window_and_zero_threshold_never_fires() {
        // 绝对阈值不依赖窗口：窗口信息缺席照样判（v2.137 前窗口未知 =
        // 百分比算不出 = 永不触发）；阈值 0 = 关自动。
        assert_eq!(
            should_auto_organize(&inp(Pacing::Batch)),
            Some(OrganizeReason::Threshold)
        );
        let mut off = inp(Pacing::Batch);
        off.threshold_tokens = 0;
        assert_eq!(should_auto_organize(&off), None);
    }

    #[test]
    fn incremental_fires_on_chunk_size_not_threshold() {
        let mut i = inp(Pacing::Incremental);
        i.anchor_tokens = 1; // 占用极低也不影响增量档
        assert_eq!(
            should_auto_organize(&i),
            Some(OrganizeReason::IncrementalChunk)
        );
        i.tail_tokens = 49_999;
        assert_eq!(should_auto_organize(&i), None);
        i.chunk_tokens = 0;
        i.tail_tokens = 60_000;
        assert_eq!(should_auto_organize(&i), None); // 配置缺失宁可不整
    }

    #[test]
    fn pacing_parse_unknown_falls_back_to_batch() {
        assert_eq!(Pacing::parse("incremental"), Pacing::Incremental);
        assert_eq!(Pacing::parse("batch"), Pacing::Batch);
        assert_eq!(Pacing::parse("nonsense"), Pacing::Batch);
    }
}
