//! 插件管理区（⑬ 自动停用设置项，exposure 文档 §8.8 唯一例外条款）：
//! "只提示，不自动执行"的默认之外，用户显式开启"N 天未使用自动停用"。
//! 缺省 0 = 关（永不自动停）；开启后只作用于安装态第三方插件——内置
//! 插件随程序分发、不参与闲置评估（与 ⑩ IdleBadge 口径一致）。

use serde::{Deserialize, Serialize};

/// 自动停用天数上下界：太短会"背着自己做事"，太长无意义。
pub const AUTO_DISABLE_MIN_DAYS: u32 = 7;
pub const AUTO_DISABLE_MAX_DAYS: u32 = 365;

/// 插件管理设置。auto_disable_idle_days = 0（缺省）表示关闭。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginsSettings {
    pub auto_disable_idle_days: u32,
}

/// 插件管理区投影（结构即设置原值）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginsView {
    pub auto_disable_idle_days: u32,
}

/// 插件管理区部分更新：字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginsUpdate {
    pub auto_disable_idle_days: Option<u32>,
}

/// 合法取值：0（关）或 7–365。
pub fn valid_days(days: u32) -> bool {
    days == 0 || (AUTO_DISABLE_MIN_DAYS..=AUTO_DISABLE_MAX_DAYS).contains(&days)
}
