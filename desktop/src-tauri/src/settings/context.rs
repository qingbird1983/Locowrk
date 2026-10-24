//! 上下文管理设置类型（specs/13 刀④ · 契约 C1）。
//!
//! 只剩**用户该负责的三格 + 一枚阈值**：自动压缩开关、用量条显隐、摘要调用预算、
//! 自动压缩阈值（**占模型窗口的百分比**）。
//!
//! **压缩阈值从"绝对 token"改成"占窗口的百分比"**：绝对数不跟窗口走 = 黑盒
//! （窗口 976K 的模型被按 100K 压、窗口 128K 的模型几乎贴边）。新口径
//! `min(窗口 × 百分比, 窗口 − 输出预留)`，缺省 **50%**。
//!
//! **退役的键**（老配置文件里那枚键**保留在文件里不报错**，但**不再被读**、
//! 设置面不显示——不删用户数据）：
//!
//! * `thresholdTokens`：绝对 token 滑杆（100K–800K）。它的语义整块换成了上面那枚
//!   百分比。
//! * `compactModel` / `compactInstructions`：摘要走**主对话那条管线**之后，
//!   "摘要用哪个模型 / 自定义压缩指令"两件事都没有对应物了（上游同款：压缩用
//!   会话的模型，提示词逐字固定）。~~留着键是为了老配置读得进来~~ →
//!   **2026-10-24 已从结构体物理删除**（收账 A6）：serde 对未知键一律忽略 ⇒
//!   老配置带着这两枚键读进来不报错、不迁移、不落盘回写。

use serde::{Deserialize, Serialize};

use super::SettingsStore;

/// 自动压缩阈值缺省（**50%**）。
///
/// 为什么不是上游那 85%：上游主力窗口 ~200K ⇒ 85% ≈ 170K；本仓主力模型窗口
/// **976K** ⇒ 85% 要 830K 才触发，等于长期不触发、白白养一个 976K 的上下文。
/// 这一条是**我们自己的取值**（specs/13「唯一保留」节）。
pub const AUTO_COMPACT_PCT_DEFAULT: u64 = 50;

/// 自动压缩阈值的三档预置（界面渲染成三档，不给数字输入框）。
///
/// **后端是校验权威**：写不进这三档的更新会被拒收（同 [`SUMMARY_BUDGET_PRESETS_MS`]
/// 那条纪律——同一条声明的**两个投影**，不是两个出处：后端这一份 + 界面那份
/// `desktop/src/lib/auto-compact-pct.ts`；漂了的表现是"选了却存不进去"或
/// "当前值不在候选里"）。
///
/// ⚠ 为什么拒绝档外值而不是只查范围：档外值只可能来自**手改过的 settings.json**
/// （这枚键是新加的，老配置里根本没有它 ⇒ 没有存量档外值要迁移）。而档外值在界面上
/// 只能"就近显示"，那个显示值与实际生效值对不上——用户看到 25% 而机器按 30% 跑。
/// 与其留一个说不清的中间态，不如把它挡在门外并让用户选一档。
pub const AUTO_COMPACT_PCT_PRESETS: [u64; 3] = [25, 50, 85];

/// 自动压缩阈值合法性（update.rs 校验用）：必须是三档之一。
pub fn valid_auto_compact_pct(v: u64) -> bool {
    AUTO_COMPACT_PCT_PRESETS.contains(&v)
}

/// 单条工具输出进入模型上下文的硬上限（24 KiB，写死）。
///
/// 取值出处：真机压测会话 `desktop-1791871305177` 的 `llm_usage.input = 630_329`，
/// 一条 `fs_read` 灌进 145 KiB 就能把单轮请求顶到 63 万 token。24 KiB ≈ 6k token，
/// 够一次正常读取或命令输出，又挡得住"一次喂一整篇"。截断只发生在**发送侧**，
/// 会话日志仍存全量（它是唯一真相源）。
pub(crate) const TOOL_RESULT_BUDGET_BYTES: usize = 24 * 1024;

/// 深度压缩开关缺省（**开**）：占用到线时把较旧的一段交给模型写成摘要。它是唯一
/// 的自动压缩层，关掉 = 完全不自动压（接近窗口只剩提醒），手动 `/压缩` 仍可用。
pub const COMPACT_ENABLED_DEFAULT: bool = true;

/// 摘要调用预算的三档（毫秒）：30s / 90s（缺省）/ 300s。
///
/// 形态是**三档预置**而不是数字输入框（S4 刀④ 的普通分区约束：用户页不出现
/// `type="number"`）——这三档是"摘要能等多久"的三种量级，不是让人填数值试验的
/// 自由量。30s 适合小上下文快速摘要，300s 留给大上下文慢模型。
pub const SUMMARY_BUDGET_PRESETS_MS: [u64; 3] = [30_000, 90_000, 300_000];

/// 摘要调用预算缺省（90s）。
///
/// 为什么要有这枚预算（specs/12 刀① · specs/13 D1）：压缩请求改走**主模型管线**
/// 之后会继承主对话轮「重试不设上限」那条裁定 ⇒ 不加预算 = 把"卡在思考中"原样
/// 请回来。预算给它一条硬性上限：超了就是一次失败，推进熔断计数。
pub const SUMMARY_BUDGET_DEFAULT_MS: u64 = 90_000;

/// 摘要调用预算合法性（update.rs 校验用）：只许三档之一。
pub fn valid_summary_budget(v: u64) -> bool {
    SUMMARY_BUDGET_PRESETS_MS.contains(&v)
}

/// `context` 区（settings.json）。字段级缺省：旧文件无此区 / 带旧键 →
/// 新键走缺省（旧键幻影），零迁移。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextSettings {
    /// 旧版窗口字段（S2 前的落点）。仅作迁移来源；新写入走 `llm` 区，
    /// open() 把它搬到 llm 后置 None。
    pub window_tokens: Option<u64>,
    /// **退役**：压缩阈值（绝对 token）。保留在文件里不报错，但**不再被读**
    /// （读数改走 [`ContextSettings::auto_compact_pct`]）。
    pub threshold_tokens: u64,
    /// 阈值用量进度条显隐（缺省开）：关掉后顶栏下方进度条整根不渲染
    /// （读数仍走用量浮层）。
    pub usage_bar: bool,
    /// 自动压缩开关（缺省开）。
    pub compact_enabled: bool,
    /// 自动压缩阈值 = **占模型窗口的百分比**（缺省 50，1–100 的整数）。
    pub auto_compact_pct: u64,
    /// 摘要调用预算（毫秒，缺省 90s，三档预置）。**总时长**（整趟共享，含全部
    /// 重试与那一次提醒重发）——主对话轮的「重试不设上限」不受它影响。
    pub summary_budget_ms: u64,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            window_tokens: None,
            threshold_tokens: COMPACT_THRESHOLD_DEFAULT,
            usage_bar: true,
            compact_enabled: COMPACT_ENABLED_DEFAULT,
            auto_compact_pct: AUTO_COMPACT_PCT_DEFAULT,
            summary_budget_ms: SUMMARY_BUDGET_DEFAULT_MS,
        }
    }
}

/// **退役**的绝对阈值缺省（只为了老配置读得进来；新代码不读它）。
///
/// 保留这一枚常量是为了让 [`ContextSettings::default`] 里的那个字段有个出处，
/// 而不是让它变成一个魔数。
pub const COMPACT_THRESHOLD_DEFAULT: u64 = 100_000;

/// 上下文管理区投影：只有界面真正渲染的那几格。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettingsView {
    /// 自动压缩阈值（**占模型窗口的百分比**）。
    pub auto_compact_pct: u64,
    /// 阈值用量进度条显隐（缺省开）。
    pub usage_bar: bool,
    /// 自动压缩开关（缺省开）。
    pub compact_enabled: bool,
    /// 摘要调用预算（毫秒；界面渲染成三档预置，不暴露毫秒数）。
    pub summary_budget_ms: u64,
}

/// 上下文管理区部分更新（settings_set 入参）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextUpdate {
    pub auto_compact_pct: Option<u64>,
    pub usage_bar: Option<bool>,
    pub compact_enabled: Option<bool>,
    pub summary_budget_ms: Option<u64>,
}

impl ContextSettings {
    /// 深度压缩一组的活配置（`compact::Compactor` 每轮取一次）。
    ///
    /// 一次取锁取齐：分几次取会在中途被用户保存插进来 ⇒ 同一轮判定吃两个版本的
    /// 设置。
    pub fn compact_config(&self) -> CompactConfig {
        CompactConfig {
            enabled: self.compact_enabled,
            auto_compact_pct: self.auto_compact_pct,
            budget_ms: self.summary_budget_ms,
        }
    }
}

/// [`ContextSettings::compact_config`] 的产物（一次锁取齐）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactConfig {
    pub enabled: bool,
    /// 自动压缩阈值 = 占模型窗口的百分比（1–100）。
    pub auto_compact_pct: u64,
    /// 摘要调用预算（毫秒）——**总时长**（含全部重试），不是单次尝试。
    pub budget_ms: u64,
}

impl ContextSettings {
    /// 校验 + 应用。越界一律拒绝而不是钳到边界——设置面写的是用户意图，
    /// 悄悄改掉等于骗人。
    pub(crate) fn apply_update(&mut self, u: ContextUpdate) -> Result<(), String> {
        if let Some(n) = u.auto_compact_pct {
            if !valid_auto_compact_pct(n) {
                return Err(format!("自动压缩阈值 {n} 超出范围（1–100 %）"));
            }
            self.auto_compact_pct = n;
        }
        if let Some(v) = u.usage_bar {
            self.usage_bar = v;
        }
        if let Some(v) = u.compact_enabled {
            self.compact_enabled = v;
        }
        if let Some(v) = u.summary_budget_ms {
            if !valid_summary_budget(v) {
                let presets = SUMMARY_BUDGET_PRESETS_MS
                    .iter()
                    .map(|p| format!("{}s", p / 1000))
                    .collect::<Vec<_>>()
                    .join(" / ");
                return Err(format!("摘要调用预算 {} 不在可选档位内（{}）", v, presets));
            }
            self.summary_budget_ms = v;
        }
        Ok(())
    }
}

/// 上下文区的**后端读面**。
impl SettingsStore {
    /// 后端专用：自动压缩阈值（占模型窗口的百分比）。
    pub fn auto_compact_pct(&self) -> u64 {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .auto_compact_pct
    }

    /// 后端专用：自动压缩一组的活配置（一次取锁取齐）。
    pub fn compact_config(&self) -> CompactConfig {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .compact_config()
    }
}
