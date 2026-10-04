//! 开发者选项区（UI shell 文档 §3.2 / §七 规则 16 可配置原则②「禁止项」
//! 的容器）：**默认隐藏**的设置分区——`unlocked` 存解锁态（侧栏版本号
//! 连点 7 次置真，安卓式；分区内可锁回），其余全是危险开关（界面上写死
//! 的禁止/限制的解锁面），缺省全关——禁止项保持写死语义，解锁是显式的
//! 开发者行为。普通用户偏好不进本区（那是各常规设置分区的职责）。

use serde::{Deserialize, Serialize};

/// 缓存命中率告警阈值缺省（%）：会话累计命中率跌破此值时提醒（M0 观测，
/// KV v2 §7.4）。
pub const CACHE_HIT_ALERT_PERCENT_DEFAULT: u8 = 80;

// ---- M2 记忆树整理（树 v2 §15 开发者层级可调项；Memory-KV Roadmap）----

/// 反弹熔断 k 缺省（§5.5：整理后 k 个用户轮内再触发 = 反弹）。
pub const ORGANIZE_REBOUND_K_DEFAULT: u32 = 3;
/// 反弹熔断 m 缺省（连续 m 次反弹 → 暂停本会话自动整理）。
pub const ORGANIZE_REBOUND_M_DEFAULT: u32 = 3;
/// 熔断 k/m 合法区间（1–10）。
pub const ORGANIZE_REBOUND_RANGE: std::ops::RangeInclusive<u32> = 1..=10;
/// 记录员单次调用超时缺省（秒）。
pub const RECORDER_TIMEOUT_SECS_DEFAULT: u64 = 60;
pub const RECORDER_TIMEOUT_SECS_RANGE: std::ops::RangeInclusive<u64> = 10..=600;
/// 记录员重试次数缺省（仍失败 → 规则切分降级，§5.4）。
pub const RECORDER_MAX_RETRIES_DEFAULT: u32 = 3;
pub const RECORDER_MAX_RETRIES_RANGE: std::ops::RangeInclusive<u32> = 0..=10;
/// 规则切分块缺省与合法区间（§15：5K–50K 缺省 20K）。
pub const RULE_CHUNK_TOKENS_DEFAULT: u64 = 20_000;
pub const RULE_CHUNK_TOKENS_RANGE: std::ops::RangeInclusive<u64> = 5_000..=50_000;
/// 会话缺省整理模式缺省值（M4 树 v2 §15「会话缺省 organizer」：阶段 2 =
/// 开发者层，缺省 legacy；阶段 3 数据裁决后才切用户层）。
pub const DEFAULT_ORGANIZER_DEFAULT: &str = "legacy";
/// 合法值（未知值保存时拒绝）。
pub const DEFAULT_ORGANIZER_VALUES: [&str; 2] = ["legacy", "tree"];

/// 开发者选项设置（settings.json `developer` 区，字段级缺省零迁移）。
/// Default = 全关是语义要求：分区隐藏 + 禁止项保持写死语义（阈值是唯一
/// 带非零缺省的字段，只在告警开启后有意义）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct DeveloperSettings {
    /// 分区解锁态：真 = 导航轨 / ⌘K / 面板三处可见。
    pub unlocked: bool,
    /// 允许删除运行中的会话（后端忙锁守卫放行）。运行轮的余段会重建日志
    /// 文件（append 语义），开关 hint 如实说明；调度最近目标同步剔除，
    /// 不向已删会话注入触发轮。
    pub allow_delete_running_session: bool,
    /// 允许切走正在运行的会话（前端守卫放行）。半截轮次会掺入新会话转录
    /// （事件不分会话过滤），收束后切回可补全。
    pub allow_switch_running_session: bool,
    /// 缓存命中率告警（缺省关）：会话累计命中率跌破阈值时 toast 一次，
    /// 每会话至多一次；≥3 个 LLM 步才判——冷启动首请求无缓存可命，
    /// 首轮低命中率不是断裂。
    pub cache_hit_alert: bool,
    /// 告警阈值（%）。
    pub cache_hit_alert_percent: u8,
    /// 记忆树反弹熔断 k（§5.5，1–10 缺省 3）。
    pub organize_rebound_k: u32,
    /// 记忆树反弹熔断 m（连续 m 次 → 暂停自动整理，1–10 缺省 3）。
    pub organize_rebound_m: u32,
    /// 记录员单次调用超时（秒，10–600 缺省 60）。
    pub recorder_timeout_secs: u64,
    /// 记录员重试次数（0–10 缺省 3；仍失败 → 规则切分）。
    pub recorder_max_retries: u32,
    /// 规则切分块（token，5K–50K 缺省 20K）。
    pub rule_chunk_tokens: u64,
    /// 会话缺省整理模式（M4 §3.38①：未显式设置会话的生效档；M5 数据
    /// 裁决后切用户层）。
    pub default_organizer: String,
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            unlocked: false,
            allow_delete_running_session: false,
            allow_switch_running_session: false,
            cache_hit_alert: false,
            cache_hit_alert_percent: CACHE_HIT_ALERT_PERCENT_DEFAULT,
            organize_rebound_k: ORGANIZE_REBOUND_K_DEFAULT,
            organize_rebound_m: ORGANIZE_REBOUND_M_DEFAULT,
            recorder_timeout_secs: RECORDER_TIMEOUT_SECS_DEFAULT,
            recorder_max_retries: RECORDER_MAX_RETRIES_DEFAULT,
            rule_chunk_tokens: RULE_CHUNK_TOKENS_DEFAULT,
            default_organizer: DEFAULT_ORGANIZER_DEFAULT.into(),
        }
    }
}

/// IPC 投影（结构即设置原值）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperView {
    pub unlocked: bool,
    pub allow_delete_running_session: bool,
    pub allow_switch_running_session: bool,
    pub cache_hit_alert: bool,
    pub cache_hit_alert_percent: u8,
    pub organize_rebound_k: u32,
    pub organize_rebound_m: u32,
    pub recorder_timeout_secs: u64,
    pub recorder_max_retries: u32,
    pub rule_chunk_tokens: u64,
    pub default_organizer: String,
}

/// 部分更新（settings_set 入参）：字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DeveloperUpdate {
    pub unlocked: Option<bool>,
    pub allow_delete_running_session: Option<bool>,
    pub allow_switch_running_session: Option<bool>,
    pub cache_hit_alert: Option<bool>,
    pub cache_hit_alert_percent: Option<u8>,
    pub organize_rebound_k: Option<u32>,
    pub organize_rebound_m: Option<u32>,
    pub recorder_timeout_secs: Option<u64>,
    pub recorder_max_retries: Option<u32>,
    pub rule_chunk_tokens: Option<u64>,
    pub default_organizer: Option<String>,
}

/// 合法性检查（update.rs 校验用）：阈值 1–100。
pub fn valid_alert_percent(v: u8) -> bool {
    (1..=100).contains(&v)
}
