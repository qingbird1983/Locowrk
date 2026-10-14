//! 外观设置（⑰ UI 视觉抛光：明暗主题 + 字号基座）。纯前端呈现偏好——后端只存取与
//! 校验，不解释（生效在前端 `lib/theme.ts` 写 `<html data-theme>`、
//! `lib/ui-font.ts` 写 `--lw-ui-font-size`，UI shell 文档 §六）。
//!
//! 字号基座为什么在这里而不在 CSS 里写死（队列 Q14 第四刀）：上位《UI Design System》
//! 附录 B 给它的落点就是设置页「外观」，而 AGENTS 可配置原则①要求「有不确定性的数值
//! 设定项一律进设置页」——相对制六档全挂在它上面，用户觉得整界面字小了本该能调。

use serde::{Deserialize, Serialize};

/// theme 取值：`light`（缺省——用户 2026-10-01 裁定，旧 settings.json 无
/// appearance 区时也随之变亮色，属预期；显式存过的值不动）/ `dark` /
/// `system`（跟随系统 prefers-color-scheme）。
pub const THEME_DARK: &str = "dark";
pub const THEME_LIGHT: &str = "light";
pub const THEME_SYSTEM: &str = "system";
const THEMES: [&str; 3] = [THEME_DARK, THEME_LIGHT, THEME_SYSTEM];

/// 字号基座的缺省与值域（队列 Q14 第四刀）。
///
/// ⚠ **缺省 14 = 改前那颗写死值**（`style.css` 里 `--lw-ui-font-size: 14px`，
/// 由 Q14 第一刀按上位附录 B 从 `--lw-fs-14` 搬来）。「可调项缺省必须等于改前的
/// 写死值」是历批定死的口径：做成可调之后，什么都没动的用户看到的尺子一个字都不该
/// 变——这条等价性由 `tests/appearance.rs` 里**一字未改**的旧用例 + 新的缺省断言共同
/// 承重（旧用例若因缺省漂移而红，就是这里改错了）。
///
/// 值域 12–18 的由来：六档是 `基座 / 基座±1 / ±2 / ±4`，12 那一档让 `xs`（= 基座−4）
/// 不小于 8px 的可读地板，18 那一档让 `xl`（= 基座+4）不超过 22px 的正文上限——
/// 两端都还在 §二⑫ 阶梯"档位随基座同尺移动"的语义内，不需要重开一档。
pub const UI_FONT_DEFAULT: u32 = 14;
pub const UI_FONT_MIN: u32 = 12;
pub const UI_FONT_MAX: u32 = 18;

/// `appearance` 区（settings.json）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceSettings {
    pub theme: String,
    /// 相对字号六档的共同基座（px）。生效点 = 前端 `lib/ui-font.ts`。
    pub ui_font_size: u32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: THEME_LIGHT.into(),
            ui_font_size: UI_FONT_DEFAULT,
        }
    }
}

/// IPC 投影（无秘钥，全量展示）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceView {
    pub theme: String,
    pub ui_font_size: u32,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceUpdate {
    pub theme: Option<String>,
    pub ui_font_size: Option<u32>,
}

/// 合法性检查（update.rs 校验用）。
pub fn valid_theme(theme: &str) -> bool {
    THEMES.contains(&theme)
}

/// 基座取值检查：超出值域一律**拒**而不是钳——"输入 99 结果界面没变"比"报错说只能
/// 12–18"更难查（同 `valid_theme` 的口径）。
pub fn valid_ui_font_size(px: u32) -> bool {
    (UI_FONT_MIN..=UI_FONT_MAX).contains(&px)
}
