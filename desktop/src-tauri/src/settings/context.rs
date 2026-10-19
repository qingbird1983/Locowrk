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

use super::SettingsStore;

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

/// 微压缩保留最近多少**组**工具结果，缺省 5（ZCode `microcompact.ts:16` 逐字）。
///
/// ⚠ **这个数字只许有一个出处**（判据 SS23a）：刀③ 时它住在 `microcompact.rs` 的
/// `const KEEP_RECENT_GROUPS`，刀⑥ 把它抬成可调项之后那颗常量必须不存在——两处一份
/// 数字就是"界面上改了、行为照旧"那种形状。缺省 = 改前那颗常量（AC-S2-10 的等价性
/// 要求），实证方式 = 微压缩那批既有用例一字未改仍然绿。
pub const MICRO_KEEP_GROUPS_DEFAULT: u64 = 5;
/// 合法区间 **1**–50。下界不取 0：0 组 = 连刚拿到的那份结果都清成占位，模型下一步
/// 就不知道自己刚读到了什么，这一层从"减速"变成"致盲"。上界 50 只是防手滑输五位数，
/// 没有语义边界（组数超过 50 的会话里这一层本来就不会动到最近这些组）。
pub const MICRO_KEEP_GROUPS_RANGE: std::ops::RangeInclusive<u64> = 1..=50;
/// 滑窗保护网保留最近多少个**过程段**，缺省 1（S2 刀⑤ 挂账 ⓕ 落地）。
/// 1 = 只留"刚做完的那一段"：熔断时模型已经拿不到摘要，这一段是它唯一知道自己刚
/// 做过什么的机会。两家参照都没有这一层，所以这颗数**是我们的裁定**、不是照抄谁。
pub const SLIDE_KEEP_RUNS_DEFAULT: u64 = 1;
/// 合法区间 **1**–20。下界不取 0 的理由与上面同一条且更硬：保护网丢掉最后一段过程
/// 之后，发出去的就是一串光秃秃的提问，模型连"用户在问第几轮"都看不出来。
pub const SLIDE_KEEP_RUNS_RANGE: std::ops::RangeInclusive<u64> = 1..=20;

/// 微压缩保留组数合法性。
pub fn valid_micro_keep_groups(v: u64) -> bool {
    MICRO_KEEP_GROUPS_RANGE.contains(&v)
}

/// 保护网保留段数合法性。
pub fn valid_slide_keep_runs(v: u64) -> bool {
    SLIDE_KEEP_RUNS_RANGE.contains(&v)
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
    /// 压缩**全局**阈值（绝对 token，缺省 100K）——裁定 B（2026-10-19）改判：它不再只管
    /// 记忆树整理，而是整条压缩面的共同分母（进度条的分母 / 微压缩那条线的母数 / 深度压缩
    /// 的触发线），界面上因此改名「压缩阈值」。
    /// ⚠ 内部标识符（`ORGANIZE_THRESHOLD_*`、`organize_threshold_tokens()`）**没跟着改**：
    /// 改名要牵动十几个消费点而零行为变化，登记为待办、不在这一刀顺手做。名字与界面不一致
    /// 会误导读代码的人，所以在这里写破一次，而不是等下一个人自己撞。
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
    /// 参数照 ZCode 逐字。刀⑥ 把「保留几组」抬成可调项（缺省仍是那颗 5，见
    /// [`MICRO_KEEP_GROUPS_DEFAULT`]），而最小节省 256 token / idle 60 分钟 / 触发线
    /// 那两式**故意不做设置项**：它们是外部读实值，且用户没法负责地调（256 token
    /// 这种量连换算都得猜）——登记在 specs/09 刀⑥ 的「故意不做」段。
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
    /// 微压缩保留最近多少组工具结果（缺省 5，ZCode 逐字）。刀⑥ 从 `microcompact.rs`
    /// 的常量抬上来，缺省 = 改前那颗数。
    pub micro_keep_groups: u64,
    /// 滑窗保护网保留最近多少段过程（缺省 1，刀⑤ 挂账 ⓕ 在这一格销账）。
    pub slide_keep_runs: u64,
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
            micro_keep_groups: MICRO_KEEP_GROUPS_DEFAULT,
            slide_keep_runs: SLIDE_KEEP_RUNS_DEFAULT,
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
    /// 微压缩保留组数（缺省 5）。
    pub micro_keep_groups: u64,
    /// 保护网保留段数（缺省 1）。
    pub slide_keep_runs: u64,
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
    pub micro_keep_groups: Option<u64>,
    pub slide_keep_runs: Option<u64>,
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
        // 刀⑥ 两枚"保留几段"的整数：越界同样拒绝（与上面几枚同一条纪律——设置面写的是
        // 用户意图，悄悄钳等于骗人）。下界都是 1，理由各写在区间常量的注释里。
        if let Some(v) = u.micro_keep_groups {
            if !super::context::valid_micro_keep_groups(v) {
                return Err(format!(
                    "微压缩保留组数 {v} 超出范围（{}–{} 组）",
                    super::context::MICRO_KEEP_GROUPS_RANGE.start(),
                    super::context::MICRO_KEEP_GROUPS_RANGE.end()
                ));
            }
            self.micro_keep_groups = v;
        }
        if let Some(v) = u.slide_keep_runs {
            if !super::context::valid_slide_keep_runs(v) {
                return Err(format!(
                    "滑窗保留段数 {v} 超出范围（{}–{} 段）",
                    super::context::SLIDE_KEEP_RUNS_RANGE.start(),
                    super::context::SLIDE_KEEP_RUNS_RANGE.end()
                ));
            }
            self.slide_keep_runs = v;
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

/// 上下文区的**后端读面**（S2 刀⑥ 自 `settings.rs` 原样搬入，一条语义未改）。
///
/// 为什么搬：这个区的类型、缺省常量、校验、投影、更新与读面从此同住一个文件——
/// 加一枚设置项只动一处，而不是"类型在子文件、读面在总入口"两头发。搬的直接起因是
/// 刀⑥ 那两枚 getter 把 `settings.rs` 顶到 419 行撞长度闸；按三问自检第③问（"改它
/// 常牵动无关部分"= 是，每加一个区都要动总入口）该拆，而**该搬的正是本区自己那一族**
/// ——把别的区的读面挪进来凑数、或为通过而放宽闸，都是这一条纪律明确禁掉的两种走法。
///
/// `inner` 是父模块的私有字段，子模块看得到（Rust 的私有对后代开放），所以这一族
/// 不必为此抬宽任何可见性。
impl SettingsStore {
    /// 后端专用：压缩全局阈值（绝对 token，v2.137 起是绝对值、不再折算窗口百分比；
    /// UsagePolicy / 记忆树批量触发 / 进度条分母同源）。名字里的 `organize` 是裁定 B
    /// 改判**之前**留下的，见 [`ContextSettings::threshold_tokens`] 那段写破的话。
    pub fn organize_threshold_tokens(&self) -> u64 {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .threshold_tokens
    }

    /// 后端专用：微压缩开关（S2 刀③，缺省开）。监听器每请求读一次，
    /// 保存即生效——它是"要不要动"的门，不是数值，锁死到会话内反而违背
    /// 可配置原则「改了就看见」。
    pub fn context_micro_enabled(&self) -> bool {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .micro_enabled
    }

    /// 后端专用：微压缩保留组数（刀⑥，缺省 = 刀③ 那颗硬编码常量 5）。与上面那枚
    /// 开关同一条"改了就看见"：监听器每请求读一次，不锁进会话。
    /// ⚠ 调用点只许一个（判据 SS23b）——同一轮里读两次会被中途保存插队。
    pub fn context_micro_keep_groups(&self) -> usize {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .micro_keep_groups as usize
    }

    /// 后端专用：滑窗保护网保留段数（刀⑥ 把刀⑤ 的挂账 ⓕ 落成可调项，缺省 1）。
    /// 同样只许一个调用点（判据 SS23b）。
    pub fn context_slide_keep_runs(&self) -> usize {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .slide_keep_runs as usize
    }

    /// 后端专用：深度压缩一组的活配置（S2 刀④c，缺省 = 刀④ 之前的硬编码常量）。
    /// **一次取锁取齐五枚**，理由与空串读成 `None` 的口径都在
    /// [`ContextSettings::compact_config`]，这里只是加锁的那一层薄壳。
    pub fn compact_config(&self) -> CompactConfig {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .compact_config()
    }

    /// 后端专用：工具结果预算（队列 Q8 · CB §7.4）——**字节**数，`None` = 不
    /// 限制。KiB→字节的换算只在本文件上面那一份里，wire / 组装点都读这里，
    /// 不再各自乘 1024（§11「同一读数只许一个出处」）。
    pub fn tool_result_budget_bytes(&self) -> Option<usize> {
        let kib = self
            .inner
            .lock()
            .expect("settings poisoned")
            .context
            .tool_result_budget_kib;
        tool_result_budget_bytes(kib)
    }
}
