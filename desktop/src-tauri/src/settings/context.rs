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

/// 深度压缩开关缺省（**开**）。它与微压缩那枚同一条理由（刀③）：这一层是**会改写
/// 发给模型的内容**的压缩，而且是三层里唯一不可逆地"把一段原文换成一份摘要"的，
/// 没有可关的那一格就不能默认开（AGENTS §4 可配置原则②）。
pub const COMPACT_ENABLED_DEFAULT: bool = true;
/// 压到有效阈值的这一成，缺省 50。⚠ 这是**我们自己的裁定**（两家参照里都没有这颗数，
/// 出处与理由见 `compact.rs` 头注），所以它必须做成可调项、界面上说破"压到多少"。
/// 缺省值 = S2 刀④ 之前 `compact.rs` 里那颗硬编码常量（AC-S2-10 的"缺省 = 现状"）。
pub const COMPACT_TARGET_PCT_DEFAULT: u64 = 50;
/// 目标比合法区间（1–100）。**100 = 压到与阈值同高**，等于"只腾出摘要那一节的位子"，
/// 允许；0 不允许（那会算出 0 预算 ⇒ 每一步都压到只剩一条消息）。
pub const COMPACT_TARGET_PCT_RANGE: std::ops::RangeInclusive<u64> = 1..=100;
/// 摘要之外保留多少 token 的近期原文，缺省 15_000（OpenCode `compaction.ts:197`
/// `keep: 15_000` 逐字）。
pub const COMPACT_KEEP_TOKENS_DEFAULT: u64 = 15_000;
/// 近期原文额度合法区间（0–400_000）。上限取 400K = 缺省整理阈值 100K 的四档，
/// 再高就装得下任何窗口，等于把这一层关掉；0 = 不留近期原文（全交给摘要，
/// 极端但用户可能想要，所以不禁）。
pub const COMPACT_KEEP_TOKENS_RANGE: std::ops::RangeInclusive<u64> = 0..=400_000;

/// 目标比合法性（`apply_update` 校验用）。
pub fn valid_compact_target_pct(v: u64) -> bool {
    COMPACT_TARGET_PCT_RANGE.contains(&v)
}

/// 近期原文额度合法性（`apply_update` 校验用）。
pub fn valid_compact_keep_tokens(v: u64) -> bool {
    COMPACT_KEEP_TOKENS_RANGE.contains(&v)
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
    /// 微压缩开关（S2 刀③，缺省**开**）：把很旧的可再生工具结果清成一行占位，
    /// 先于主层压缩触发。开关系放在这一刀是因为它是**第一个会改变发给模型内容**
    /// 的压缩层——没有可关的那一格就不能默认开（AGENTS §4 可配置原则②）。
    /// 参数仍照 ZCode 逐字（保留 5 组 / 最小节省 256 / idle 60 分钟），数值设置化归刀⑥。
    pub micro_enabled: bool,
    /// 深度压缩开关（S2 刀④c，缺省**开**）：占用到有效阈值时把较旧的一段交给
    /// 模型写成摘要。这是三层里唯一"一段原文换一份摘要"的、不可逆（对上下文而言）
    /// 的一层，所以它自己有开关；关掉 = 完全不压，接近窗口只剩提醒（`llm/retry.rs`）。
    pub compact_enabled: bool,
    /// 压到有效阈值的这一成（缺省 50，**我们的裁定**，出处见 `compact.rs` 头注）。
    pub compact_target_pct: u64,
    /// 摘要之外保留多少 token 的近期原文（缺省 15_000，OpenCode 逐字）。
    pub compact_keep_tokens: u64,
    /// 摘要调用用哪个模型（空 = 不覆盖，走记忆管理器档位那条路由 → 主模型）。
    /// 路由只有一处实现（判据 SS19c），这一枚只是它的一个入参。
    pub compact_model: String,
    /// 自定义压缩指令（空 = 与没设同形）。插在四段骨架之后、尾部那次"只用文本回答"
    /// 之前（用例 `custom_instructions_are_appended_only_when_meaningful` 钉住）。
    pub compact_instructions: String,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            window_tokens: None,
            threshold_tokens: ORGANIZE_THRESHOLD_DEFAULT,
            usage_bar: true,
            tool_result_budget_kib: TOOL_RESULT_BUDGET_DEFAULT_KIB,
            micro_enabled: true,
            compact_enabled: COMPACT_ENABLED_DEFAULT,
            compact_target_pct: COMPACT_TARGET_PCT_DEFAULT,
            compact_keep_tokens: COMPACT_KEEP_TOKENS_DEFAULT,
            compact_model: String::new(),
            compact_instructions: String::new(),
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
    /// 微压缩开关（缺省开）。
    pub micro_enabled: bool,
    /// 深度压缩开关（缺省开）。
    pub compact_enabled: bool,
    /// 压到有效阈值的这一成（缺省 50）。
    pub compact_target_pct: u64,
    /// 摘要之外保留多少 token 的近期原文（缺省 15_000）。
    pub compact_keep_tokens: u64,
    /// 摘要模型（空 = 跟随路由）。
    pub compact_model: String,
    /// 自定义压缩指令（空 = 不加）。
    pub compact_instructions: String,
}

/// 上下文管理区部分更新（settings_set 入参）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextUpdate {
    pub threshold_tokens: Option<u64>,
    pub usage_bar: Option<bool>,
    pub tool_result_budget_kib: Option<u64>,
    pub micro_enabled: Option<bool>,
    pub compact_enabled: Option<bool>,
    pub compact_target_pct: Option<u64>,
    pub compact_keep_tokens: Option<u64>,
    pub compact_model: Option<String>,
    pub compact_instructions: Option<String>,
}

impl ContextSettings {
    /// 深度压缩一组的活配置（S2 刀④c，`compact::Compactor` 每轮取一次）。
    ///
    /// 为什么是**一个结构**而不是五枚 getter：判定一次要用这五枚一起决定"压不压、
    /// 压到哪、谁写摘要"，分五次取锁会在中途被用户保存插进来 ⇒ 同一轮判定吃两个
    /// 版本的设置（`settings.rs` 里 `with_llm_config` 那条"一次取锁"批注说的同一件事）。
    /// 空串在存进磁盘时就被 trim 掉了，这里只需把空读成 `None`（= 未设 = 走缺省路由 /
    /// 不加附加指令）。
    pub fn compact_config(&self) -> CompactConfig {
        let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
        CompactConfig {
            enabled: self.compact_enabled,
            target_pct: self.compact_target_pct,
            keep_tokens: self.compact_keep_tokens,
            model: opt(&self.compact_model),
            instructions: opt(&self.compact_instructions),
        }
    }
}

/// [`ContextSettings::compact_config`] 的产物（一次锁取齐）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactConfig {
    pub enabled: bool,
    pub target_pct: u64,
    pub keep_tokens: u64,
    pub model: Option<String>,
    pub instructions: Option<String>,
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
        if let Some(v) = u.micro_enabled {
            self.micro_enabled = v;
        }
        // S2 刀④c 深度压缩五键。数值两枚越界一律拒绝（与工具结果预算同一条：设置面
        // 写的是用户意图，悄悄钳等于骗人）；文本两枚**写入即 trim**，所以"空 = 没设"
        // 这一档在磁盘上就成立（读面不必再判空白，界面清掉内容后行为立刻回到缺省）。
        if let Some(v) = u.compact_enabled {
            self.compact_enabled = v;
        }
        if let Some(v) = u.compact_target_pct {
            if !super::context::valid_compact_target_pct(v) {
                return Err(format!(
                    "压缩目标比 {v} 超出范围（{}–{}，单位 %）",
                    super::context::COMPACT_TARGET_PCT_RANGE.start(),
                    super::context::COMPACT_TARGET_PCT_RANGE.end()
                ));
            }
            self.compact_target_pct = v;
        }
        if let Some(v) = u.compact_keep_tokens {
            if !super::context::valid_compact_keep_tokens(v) {
                return Err(format!(
                    "压缩保留额度 {v} 超出范围（{}–{} token）",
                    super::context::COMPACT_KEEP_TOKENS_RANGE.start(),
                    super::context::COMPACT_KEEP_TOKENS_RANGE.end()
                ));
            }
            self.compact_keep_tokens = v;
        }
        if let Some(v) = u.compact_model {
            self.compact_model = v.trim().to_string();
        }
        if let Some(v) = u.compact_instructions {
            self.compact_instructions = v.trim().to_string();
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
