//! 上下文管理设置类型（⑨，Context and Memory.md §3 / §6.2）。
//! 独立成文件遵守 400 行纪律（memory/tools/shell 同款）。
//!
//! S2（清单 §5 信息归属）：模型上下文窗口是**模型的属性**，字段已迁
//! `llm` 区；本区只留阈值（百分比）与策略。`window_tokens` 字段保留
//! 仅为读旧 settings.json（open() 迁移一次到 llm 区），不再进投影
//! 与更新入参。

use kernel_agent::{suggested_threshold, CompactStrategy};
use serde::{Deserialize, Serialize};

/// strategy 与 kernel-agent 的 CompactStrategy 同形（serde snake_case
/// 直接复用）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextSettings {
    /// 旧版窗口字段（S2 前的落点）。仅作迁移来源；新写入走 `llm` 区，
    /// open() 把它搬到 llm 后置 None。
    pub window_tokens: Option<u64>,
    /// 压缩触发阈值（窗口百分比，10–90）。
    pub threshold_pct: u32,
    /// 压缩策略（缺省分层保留）。
    pub strategy: CompactStrategy,
    /// 稳定期轮数（KV v2 §6.4，缺省 3，合法 1–10）：结算后 N 个用户轮
    /// 内不再触发压缩——新前缀尽快落盘，二次压缩只烧钱。
    pub stability_rounds: u32,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            window_tokens: None,
            threshold_pct: 30,
            strategy: CompactStrategy::default(),
            stability_rounds: 3,
        }
    }
}

impl ContextSettings {
    /// 生效阈值（绝对 token）＝ 窗口 × 百分比 / 100。窗口由调用方解析
    /// （llm 区优先、未知回落 FALLBACK_WINDOW_TOKENS）。压缩判定、进度条、
    /// 设置面板三处共用一把尺。
    pub fn effective_threshold(&self, window: u64) -> u64 {
        window * self.threshold_pct as u64 / 100
    }

    /// 窗口已知时的建议阈值百分比（§3.2.3 动态建议）；未知返回 None。
    pub fn suggested_pct(&self, window: Option<u64>) -> Option<u32> {
        window.map(suggested_threshold)
    }
}

/// 上下文管理区投影：设置原值 + 生效值（阈值绝对数、建议百分比）
/// 一起给出，前端不做第二套换算。窗口不在本区（S2 迁 llm 区）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettingsView {
    pub threshold_pct: u32,
    pub strategy: CompactStrategy,
    /// 稳定期轮数（KV v2 §6.4，1–10，缺省 3）。
    pub stability_rounds: u32,
    /// 生效阈值（窗口已知 = 窗口×百分比；未知 = FALLBACK_WINDOW_TOKENS）。
    pub threshold_tokens: u64,
    /// 建议阈值百分比（窗口未知 = None，UI 隐藏建议行）。
    pub suggested_pct: Option<u32>,
}

/// 上下文管理区部分更新（settings_set 入参）。S2 起不再收窗口——
/// 窗口走 `LlmUpdate.window_tokens`。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextUpdate {
    pub threshold_pct: Option<u32>,
    pub strategy: Option<CompactStrategy>,
    pub stability_rounds: Option<u32>,
}

/// stability_rounds 合法区间（KV v2 §9 可调项总表）：1–10。下限 1 保住
/// "结算当轮不二次结算"的最低纪律，上限 10 防稳定期变成禁压。
pub const STABILITY_ROUNDS_RANGE: std::ops::RangeInclusive<u32> = 1..=10;
