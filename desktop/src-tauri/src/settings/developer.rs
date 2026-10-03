//! 开发者选项区（UI shell 文档 §3.2 / §七 规则 16 可配置原则②「禁止项」
//! 的容器）：**默认隐藏**的设置分区——`unlocked` 存解锁态（侧栏版本号
//! 连点 7 次置真，安卓式；分区内可锁回），其余全是危险开关（界面上写死
//! 的禁止/限制的解锁面），缺省全关——禁止项保持写死语义，解锁是显式的
//! 开发者行为。普通用户偏好不进本区（那是各常规设置分区的职责）。

use serde::{Deserialize, Serialize};

/// 缓存命中率告警阈值缺省（%）：会话累计命中率跌破此值时提醒（M0 观测，
/// KV v2 §7.4）。
pub const CACHE_HIT_ALERT_PERCENT_DEFAULT: u8 = 80;

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
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            unlocked: false,
            allow_delete_running_session: false,
            allow_switch_running_session: false,
            cache_hit_alert: false,
            cache_hit_alert_percent: CACHE_HIT_ALERT_PERCENT_DEFAULT,
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
}

/// 合法性检查（update.rs 校验用）：阈值 1–100。
pub fn valid_alert_percent(v: u8) -> bool {
    (1..=100).contains(&v)
}
