//! 外观设置（⑰ UI 视觉抛光：明暗主题）。纯前端呈现偏好——后端只存取与
//! 校验，不解释（生效在前端 `lib/theme.ts` 写 `<html data-theme>`，UI
//! shell 文档 §六）。

use serde::{Deserialize, Serialize};

/// theme 取值：`light`（缺省——用户 2026-10-01 裁定，旧 settings.json 无
/// appearance 区时也随之变亮色，属预期；显式存过的值不动）/ `dark` /
/// `system`（跟随系统 prefers-color-scheme）。
pub const THEME_DARK: &str = "dark";
pub const THEME_LIGHT: &str = "light";
pub const THEME_SYSTEM: &str = "system";
const THEMES: [&str; 3] = [THEME_DARK, THEME_LIGHT, THEME_SYSTEM];

/// `appearance` 区（settings.json）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceSettings {
    pub theme: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: THEME_LIGHT.into(),
        }
    }
}

/// IPC 投影（无秘钥，全量展示）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceView {
    pub theme: String,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceUpdate {
    pub theme: Option<String>,
}

/// 合法性检查（update.rs 校验用）。
pub fn valid_theme(theme: &str) -> bool {
    THEMES.contains(&theme)
}
