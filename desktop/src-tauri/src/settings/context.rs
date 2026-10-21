//! 上下文管理设置类型（S4 退役面 · specs/11 刀①）。
//!
//! 只剩**用户该负责的三格**：压缩阈值（档位滑条）、自动压缩开关、用量条显隐。
//! 其余数值全部写死进代码：输出预留 21K、目标比 50%、近期原文 15K、单条工具输出
//! 24 KiB。它们不是用户能负责地"填"的量——填大了撞窗、填小了白花钱，故不进设置面
//! （决策人 2026-10-20 原话：「哪有自己填数值来试验的」）。
//!
//! S2/S3 期间抬上界面的微压缩开关/保留组数、滑窗保留段数、输出预留上限、目标比、
//! 近期原文额度全部随各自的层退役；settings.json 里的历史键是**幻影键**（serde
//! 未知字段忽略，读取不破，下次保存自然消失）。

use serde::{Deserialize, Serialize};

use super::SettingsStore;

/// 压缩阈值缺省（100K）：主流大窗口模型安全（需低于模型窗口，否则上下文
/// 超限会先于压缩发生——设置行提示同文）。
pub const COMPACT_THRESHOLD_DEFAULT: u64 = 100_000;
/// 压缩阈值合法区间（100K–800K，100K 一档；UI 滑块同档位）。
pub const COMPACT_THRESHOLD_RANGE: std::ops::RangeInclusive<u64> = 100_000..=800_000;

/// 压缩阈值合法性（update.rs 校验用）。
pub fn valid_compact_threshold(v: u64) -> bool {
    COMPACT_THRESHOLD_RANGE.contains(&v)
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
/// 为什么要有这枚预算（specs/12 刀①）：摘要这条辅助路径曾继承主对话轮
/// 「重试不设上限」那条裁定，却**没有继承停止能力**——摘要请求一失败就永远
/// 在那儿转，界面表现是卡在「思考中」，且按「停止生成」也没用。预算给它一条
/// 硬性上限：超了就是一次失败，推进熔断计数，主轮继续。
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
    /// 压缩阈值（绝对 token，缺省 100K）。它是整条压缩面的共同分母：
    /// 进度条的分母、深度压缩的触发线、手动 `/压缩` 的目标位同源。
    pub threshold_tokens: u64,
    /// 阈值用量进度条显隐（缺省开）：关掉后顶栏下方进度条整根不渲染
    /// （读数仍走用量浮层）。
    pub usage_bar: bool,
    /// 自动压缩开关（缺省开）。
    pub compact_enabled: bool,
    /// 摘要调用用哪个模型（空 = 走主模型）。不进界面：它属于模型接入面，
    /// 不是普通用户该选的压缩参数。
    pub compact_model: String,
    /// 自定义压缩指令（空 = 不用）。不进界面：多行输入是未批准的新形态。
    pub compact_instructions: String,
    /// 摘要调用预算（毫秒，缺省 90s，三档预置）。只作用于**辅助路径**（压缩
    /// 摘要）——主对话轮的「重试不设上限」不受它影响（specs/12 刀①）。
    pub summary_budget_ms: u64,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            window_tokens: None,
            threshold_tokens: COMPACT_THRESHOLD_DEFAULT,
            usage_bar: true,
            compact_enabled: COMPACT_ENABLED_DEFAULT,
            compact_model: String::new(),
            compact_instructions: String::new(),
            summary_budget_ms: SUMMARY_BUDGET_DEFAULT_MS,
        }
    }
}

/// 上下文管理区投影：只有界面真正渲染的三格（外加两个后端文本字段不投影）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettingsView {
    /// 压缩阈值（绝对 token）。
    pub threshold_tokens: u64,
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
    pub threshold_tokens: Option<u64>,
    pub usage_bar: Option<bool>,
    pub compact_enabled: Option<bool>,
    pub summary_budget_ms: Option<u64>,
}

impl ContextSettings {
    /// 深度压缩一组的活配置（`compact::Compactor` 每轮取一次）。
    ///
    /// 一次取锁取齐：分几次取会在中途被用户保存插进来 ⇒ 同一轮判定吃两个版本的
    /// 设置。空串在存进磁盘时就被 trim 掉了，这里只需把空读成 `None`。
    pub fn compact_config(&self) -> CompactConfig {
        let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
        CompactConfig {
            enabled: self.compact_enabled,
            model: opt(&self.compact_model),
            instructions: opt(&self.compact_instructions),
            budget_ms: self.summary_budget_ms,
        }
    }
}

/// [`ContextSettings::compact_config`] 的产物（一次锁取齐）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactConfig {
    pub enabled: bool,
    pub model: Option<String>,
    pub instructions: Option<String>,
    /// 摘要调用预算（毫秒）——**总时长**（含全部重试），不是单次尝试。
    pub budget_ms: u64,
}

impl ContextSettings {
    /// 校验 + 应用。越界一律拒绝而不是钳到边界——设置面写的是用户意图，
    /// 悄悄改掉等于骗人。
    pub(crate) fn apply_update(&mut self, u: ContextUpdate) -> Result<(), String> {
        if let Some(n) = u.threshold_tokens {
            if !valid_compact_threshold(n) {
                return Err(format!(
                    "压缩阈值 {n} 超出范围（{}–{} token）",
                    COMPACT_THRESHOLD_RANGE.start(),
                    COMPACT_THRESHOLD_RANGE.end()
                ));
            }
            self.threshold_tokens = n;
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
    /// 后端专用：压缩阈值（绝对 token）。UsagePolicy / 进度条分母 / 压缩触发线同源。
    pub fn compact_threshold_tokens(&self) -> u64 {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .threshold_tokens
    }

    /// 后端专用：自动压缩一组的活配置（一次取锁取齐三枚）。
    pub fn compact_config(&self) -> CompactConfig {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .compact_config()
    }
}
