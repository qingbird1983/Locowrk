//! 上下文管理设置类型（⑨，Context and Memory.md §3 / §6.2）。
//!
//! v2.137（用户裁定「上下文根本不压缩了」）：整理阈值改**绝对 token**
//! ——「对话上下文达到多少 K 就触发记忆树整理」，不再按窗口百分比折算；
//! `threshold_pct`（百分比）/ `strategy`（压缩策略三档）/ `stability_rounds`
//! （ctx-compact 稳定期）三键随 ctx-compact 桌面链一并退役，settings.json
//! 里的历史键为**幻影键**（serde 未知字段忽略，读取不破，下次保存自然
//! 消失）。S2（清单 §5 信息归属）：模型上下文窗口是**模型的属性**，字段
//! 在 `llm` 区；`window_tokens` 保留仅为读旧 settings.json（open() 迁移
//! 一次到 llm 区），不进投影与更新入参。

use serde::{Deserialize, Serialize};

/// 整理阈值缺省（100K）：主流大窗口模型安全（需低于模型窗口，否则上下文
/// 超限会先于整理发生——设置行提示同文）。
pub const ORGANIZE_THRESHOLD_DEFAULT: u64 = 100_000;
/// 整理阈值合法区间（100K–800K，100K 一档；UI 滑块同档位）。
pub const ORGANIZE_THRESHOLD_RANGE: std::ops::RangeInclusive<u64> = 100_000..=800_000;

/// 整理阈值合法性（update.rs 校验用）。
pub fn valid_organize_threshold(v: u64) -> bool {
    ORGANIZE_THRESHOLD_RANGE.contains(&v)
}

/// `context` 区（settings.json）。字段级缺省：旧文件无此区 / 带旧键 →
/// 新键走缺省（旧键幻影），零迁移。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextSettings {
    /// 旧版窗口字段（S2 前的落点）。仅作迁移来源；新写入走 `llm` 区，
    /// open() 把它搬到 llm 后置 None。
    pub window_tokens: Option<u64>,
    /// 整理阈值（绝对 token，缺省 100K）：对话上下文达到该 token 数触发
    /// 记忆树整理（批量档；增量的第二把尺在 memory.organizeChunkTokens）。
    pub threshold_tokens: u64,
    /// 阈值用量进度条显隐（UI shell §3.4 v4.18，缺省开；v2.137 改名——
    /// 分母 = 整理阈值非窗口）：关掉后顶栏下方进度条整根不渲染（读数仍
    /// 走用量浮层）。
    pub usage_bar: bool,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            window_tokens: None,
            threshold_tokens: ORGANIZE_THRESHOLD_DEFAULT,
            usage_bar: true,
        }
    }
}

/// 上下文管理区投影（v2.137 起只两键——阈值即绝对值，无换算无建议值，
/// 前端不做第二套算术）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettingsView {
    /// 整理阈值（绝对 token）。
    pub threshold_tokens: u64,
    /// 阈值用量进度条显隐（缺省开）。
    pub usage_bar: bool,
}

/// 上下文管理区部分更新（settings_set 入参）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextUpdate {
    pub threshold_tokens: Option<u64>,
    pub usage_bar: Option<bool>,
}
