//! 上下文管理设置类型（⑨，Context and Memory.md §3 / §6.2）。
//! 独立成文件遵守 400 行纪律（memory/tools/shell 同款）。

use kernel_agent::{CompactStrategy, suggested_threshold};
use serde::{Deserialize, Serialize};

use super::FALLBACK_WINDOW_TOKENS;

/// strategy 与 kernel-agent 的 CompactStrategy 同形（serde snake_case
/// 直接复用）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextSettings {
    /// 模型上下文窗口（token）。genai 不暴露该元数据 → 用户在设置面板
    /// 选择；None = 未设置，阈值回落 FALLBACK_WINDOW_TOKENS 保守保护。
    pub window_tokens: Option<u64>,
    /// 压缩触发阈值（窗口百分比，10–90）。
    pub threshold_pct: u32,
    /// 压缩策略（缺省分层保留）。
    pub strategy: CompactStrategy,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            window_tokens: None,
            threshold_pct: 30,
            strategy: CompactStrategy::default(),
        }
    }
}

impl ContextSettings {
    /// 生效阈值（绝对 token）＝ 窗口 × 百分比 / 100；窗口未知回落
    /// FALLBACK_WINDOW_TOKENS。压缩判定、进度条、设置面板三处共用一把尺。
    pub fn effective_threshold(&self) -> u64 {
        let window = self.window_tokens.unwrap_or(FALLBACK_WINDOW_TOKENS);
        window * self.threshold_pct as u64 / 100
    }

    /// 窗口已知时的建议阈值百分比（§3.2.3 动态建议）；未知返回 None。
    pub fn suggested_pct(&self) -> Option<u32> {
        self.window_tokens.map(suggested_threshold)
    }
}

/// 上下文管理区投影：设置原值 + 生效值（阈值绝对数、建议百分比）
/// 一起给出，前端不做第二套换算。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettingsView {
    pub window_tokens: Option<u64>,
    pub threshold_pct: u32,
    pub strategy: CompactStrategy,
    /// 生效阈值（窗口已知 = 窗口×百分比；未知 = FALLBACK_WINDOW_TOKENS）。
    pub threshold_tokens: u64,
    /// 建议阈值百分比（窗口未知 = None，UI 隐藏建议行）。
    pub suggested_pct: Option<u32>,
}

/// 上下文管理区部分更新（settings_set 入参）。window_tokens 用双层
/// Option：null = 清除（回落保守保护），缺省 = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextUpdate {
    pub window_tokens: Option<Option<u64>>,
    pub threshold_pct: Option<u32>,
    pub strategy: Option<CompactStrategy>,
}
