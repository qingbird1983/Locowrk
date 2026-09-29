//! 展示元数据投影（packaging.md §5.3）：从版本目录的 manifest.json 读出
//! UI 渲染卡片所需的展示字段，图标经 `IconLoader` 转 data URL。
//!
//! 纪律（§7 Fallback 矩阵）：manifest.json 缺失或解析失败都返回缺省
//! `PluginDisplay`（origin = community 安全默认）并只记 warn——任何一个
//! 字段的问题都不能让列表崩。

use std::collections::BTreeMap;
use std::path::Path;

use plugin_api::manifest::{Author, PluginManifest, PluginOrigin};
use plugin_api::tool::{SideEffect, ToolSpec};
use serde::Serialize;

use plugin_host::IconLoader;

/// UI 展示投影（§5.3 PluginUiState 的展示切片）。随 InstalledPlugin /
/// MarketplaceItem 一起序列化给前端；`display_name` / `description` 保留
/// 原始多语言 map，由前端按当前语言 `pickLocalized`。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PluginDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<BTreeMap<String, String>>,
    /// data URL（svg/png/webp，≤1MB）；None = 前端首字母兜底。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<Author>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// 来源分类（§2.4）：只影响展示分组，不是信任边界；缺省 community。
    pub origin: PluginOrigin,
    /// 工具副作用去重清单（pure → compensable → irreversible 固定序），
    /// 前端渲染徽章（§6.4）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<String>,
}

/// 读取 `dir/manifest.json` 构造展示投影。dir 不存在 / manifest 损坏
/// 都降级为缺省投影（不崩列表）。
pub fn read_display(dir: &Path, icons: &IconLoader) -> PluginDisplay {
    let path = dir.join("manifest.json");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            tracing::debug!(path = %path.display(), "no manifest for display: {e}");
            return PluginDisplay::default();
        }
    };
    let manifest = match PluginManifest::from_json(&bytes) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(path = %path.display(), "manifest unreadable, display degraded: {e}");
            return PluginDisplay::default();
        }
    };
    PluginDisplay {
        display_name: manifest.display_name.map(|l| l.0),
        description: manifest.description.map(|d| d.into_localized().0),
        icon: manifest.icon.and_then(|rel| icons.load(dir, &rel)),
        author: manifest.author,
        homepage: manifest.homepage,
        license: manifest.license,
        origin: manifest.origin,
        side_effects: side_effect_kinds(&manifest.tools),
    }
}

/// 全部工具的副作用去重清单，按 pure → compensable → irreversible 固定序
/// （徽章渲染顺序确定）。
fn side_effect_kinds(tools: &[ToolSpec]) -> Vec<String> {
    let kind = |se: &SideEffect| match se {
        SideEffect::Pure => "pure",
        SideEffect::Compensable { .. } => "compensable",
        SideEffect::Irreversible => "irreversible",
    };
    ["pure", "compensable", "irreversible"]
        .into_iter()
        .filter(|k| tools.iter().any(|t| kind(&t.side_effect) == *k))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;
