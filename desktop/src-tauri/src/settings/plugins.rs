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

impl PluginsSettings {
    /// 校验 + 应用（台账 T15 从 `settings/update.rs` **机械搬移**，编排层只留
    /// 一行转发）。搬移只做两条变换：`next.plugins.` → `self.`、缩进减一层，
    /// 其余逐字节不动（`super::plugins::` 在本模块内解析到自身，故原样保留）。
    /// 等价性证据 = 既有 settings 测试面一字未改仍然绿 + 行集合差脚本为空。
    ///
    /// 签名吃**消费型值**（与 `chat::apply_update` 同形，不是 `developer` 那个 `&u`）：
    /// 本批十个分区里有 12 处 `String` / `Vec<String>` 字段是"从 u 里搬出来落值"，
    /// 改成 `&u` 就得给每一处补 `.clone()`——那是**新写的代码**而不是搬移，
    /// 等价性从此只能靠人读。分区的 `apply_update` 本来就是这条通路的唯一消费者。
    pub(crate) fn apply_update(&mut self, u: PluginsUpdate) -> Result<(), String> {
        if let Some(days) = u.auto_disable_idle_days {
            if !super::plugins::valid_days(days) {
                return Err(format!(
                    "自动停用天数 {days} 非法（0 = 关闭，或 {}–{} 天）",
                    super::plugins::AUTO_DISABLE_MIN_DAYS,
                    super::plugins::AUTO_DISABLE_MAX_DAYS
                ));
            }
            self.auto_disable_idle_days = days;
        }
        Ok(())
    }
}
