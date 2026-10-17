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

/// 工具结果预算缺省（24 KiB）：**一条工具输出进入模型上下文的上限**（队列 Q8 ·
/// 《Context Budget Closure》§7.4）。取值不是拍的——真机压测会话
/// `desktop-1791871305177` 的 `llm_usage.input = 630_329`：一条 `fs_read` 灌进
/// 145 KiB 就能把单轮请求顶到 63 万 token，而整理只在轮末起步，于是"每轮全量
/// 重发、慢到像卡死"。24 KiB ≈ 6k token（换算走全仓唯一粗估尺
/// `middleware/compact.rs` 的 字节/4），够一次正常读取或命令输出，又挡得住
/// "一次喂一整篇"。
pub const TOOL_RESULT_BUDGET_DEFAULT_KIB: u64 = 24;
/// 工具结果预算合法区间（0–512 KiB）。**0 = 不限制**——可配置原则②要求界面上
/// 写死的限制必须有开关可关（关掉就是现状行为：源端各家 cap 之外不再加闸）。
/// 上限取 512：再高就约等于关掉这一层，且会顶穿整理阈值（缺省阈值 100K token
/// 折合 400 KiB），让"整理先于溢出"这个前提失效。
pub const TOOL_RESULT_BUDGET_RANGE: std::ops::RangeInclusive<u64> = 0..=512;

/// 工具结果预算合法性（update.rs 校验用）。
pub fn valid_tool_result_budget(v: u64) -> bool {
    TOOL_RESULT_BUDGET_RANGE.contains(&v)
}

/// KiB → 字节的**唯一**换算处（0 = 不限制 ⇒ `None`）。前端不做第二套算术，
/// wire 层也不自己乘 1024——同一读数只许一个出处。
pub fn tool_result_budget_bytes(kib: u64) -> Option<usize> {
    if kib == 0 {
        None
    } else {
        Some((kib as usize) * 1024)
    }
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
    /// 工具结果预算（KiB，缺省 24，0 = 不限制）：单条工具输出进模型上下文
    /// 的上限，截断只发生在**发送侧**——会话日志仍存全量（它是唯一真相源，
    /// 回捞 / 导出 / 崩溃重放都读它）。
    pub tool_result_budget_kib: u64,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            window_tokens: None,
            threshold_tokens: ORGANIZE_THRESHOLD_DEFAULT,
            usage_bar: true,
            tool_result_budget_kib: TOOL_RESULT_BUDGET_DEFAULT_KIB,
        }
    }
}

/// 上下文管理区投影（v2.137 起阈值即绝对值，无换算无建议值；Q8 加第三键
/// 工具结果预算——前端不做第二套算术，0 显示成「不限制」由 UI 文案承担）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettingsView {
    /// 整理阈值（绝对 token）。
    pub threshold_tokens: u64,
    /// 阈值用量进度条显隐（缺省开）。
    pub usage_bar: bool,
    /// 工具结果预算（KiB，0 = 不限制）。
    pub tool_result_budget_kib: u64,
}

/// 上下文管理区部分更新（settings_set 入参）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextUpdate {
    pub threshold_tokens: Option<u64>,
    pub usage_bar: Option<bool>,
    pub tool_result_budget_kib: Option<u64>,
}

impl ContextSettings {
    /// 校验 + 应用（台账 T15 从 `settings/update.rs` **机械搬移**：两条变换 =
    /// `next.context.` → `self.`、缩进减一层，其余逐字节不动；签名吃消费型值，
    /// 理由与等价性证据同 `plugins.rs`）。
    pub(crate) fn apply_update(&mut self, u: ContextUpdate) -> Result<(), String> {
        // v2.137 整理阈值（绝对 token，100K–800K）：折算键
        // threshold_pct / strategy / stability_rounds 随 ctx-compact
        // 桌面链退役（settings 里为幻影键）。
        if let Some(n) = u.threshold_tokens {
            if !super::context::valid_organize_threshold(n) {
                return Err(format!(
                    "整理阈值 {n} 超出范围（{}–{} token）",
                    super::context::ORGANIZE_THRESHOLD_RANGE.start(),
                    super::context::ORGANIZE_THRESHOLD_RANGE.end()
                ));
            }
            self.threshold_tokens = n;
        }
        if let Some(v) = u.usage_bar {
            self.usage_bar = v;
        }
        // Q8 工具结果预算（0–512 KiB，**0 = 不限制**）：越界一律拒绝而不是
        // 钳到边界——`plugin-api/args.rs` 那条"钳而不报错"是**工具入参**的
        // 纪律，设置面写的是用户意图，悄悄改掉等于骗人（他给什么就该存什么）。
        if let Some(v) = u.tool_result_budget_kib {
            if !super::context::valid_tool_result_budget(v) {
                return Err(format!(
                    "工具结果预算 {v} 超出范围（{}–{} KiB，0 = 不限制）",
                    super::context::TOOL_RESULT_BUDGET_RANGE.start(),
                    super::context::TOOL_RESULT_BUDGET_RANGE.end()
                ));
            }
            self.tool_result_budget_kib = v;
        }
        Ok(())
    }
}
