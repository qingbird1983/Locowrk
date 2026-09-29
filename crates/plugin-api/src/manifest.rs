//! 插件元数据契约（DESIGN.md §5.2 + docs/design/Plug-in packaging.md v2）。
//! JSON 是传输形态：describe 符号返回 NUL 结尾 UTF-8 JSON（长度协议见
//! `ffi::DescribeFn`），安装目录里的 manifest.json（开发态布局见
//! plugins/README.md）读出的也是同一份结构。
//!
//! v2 兼容纪律（packaging.md §9）：
//! - 新字段全部可选（`#[serde(default)]`），v1 manifest 照常解析；
//!   None 序列化时省略，旧插件 describe 字节不变，`abi_version` 仍为 1
//! - `description` 双形态：v1 纯字符串 / v2 多语言 map（`LocalizedOrString`）
//! - 工具级字段（如 display_name）与本结构的未知字段 serde 默认忽略，
//!   文档式 manifest 照常解析；packaging.md §2.2 的 manifest 级
//!   `reversible` 不设字段——Locowrk 的副作用模型就是按工具声明（§5.2），
//!   不需要插件级冗余
//! - `manifest_version` 不落字段：`LocalizedOrString` 已透明兼容两代格式

use std::collections::BTreeMap;
use std::ffi::CString;

use serde::{Deserialize, Serialize};

use crate::tool::ToolSpec;

/// 插件类别（§5.2）。阶段三只落地 Tool；其余类别是占位枚举值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    Tool,
    Middleware,
    Memory,
    Model,
}

/// 本地化字符串：BCP-47 语言标签 → 展示文本（packaging.md §2.3）。
/// BTreeMap（而非文档示例的 HashMap）只为确定性：同主语言多个变体
/// （zh-TW / zh-HK）并存时取字典序最小 key，`pick` 结果可测试。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalizedString(pub BTreeMap<String, String>);

impl LocalizedString {
    /// 回退链（packaging.md §2.3）：精确 → 主语言（zh-CN → zh）→ en-US
    /// → 任意（字典序最小）。全空返回 None，调用方兜底到 id（§7 矩阵）。
    pub fn pick(&self, locale: &str) -> Option<&str> {
        if let Some(s) = self.0.get(locale) {
            return Some(s);
        }
        let primary = locale.split('-').next().unwrap_or(locale);
        if let Some((_, v)) = self
            .0
            .iter()
            .find(|(k, _)| k.split('-').next() == Some(primary))
        {
            return Some(v);
        }
        if let Some(s) = self.0.get("en-US") {
            return Some(s);
        }
        self.0.values().next().map(String::as_str)
    }
}

/// 双形态字段（packaging.md §9.3 兼容矩阵）：v1 的纯字符串或 v2 的
/// 多语言 map。untagged 先按字符串试，失败再按 map。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LocalizedOrString {
    Plain(String),
    Localized(LocalizedString),
}

impl LocalizedOrString {
    /// 归一成多语言形态（纯字符串视为 en-US 单条）。
    pub fn into_localized(self) -> LocalizedString {
        match self {
            Self::Plain(s) => LocalizedString(BTreeMap::from([("en-US".to_owned(), s)])),
            Self::Localized(l) => l,
        }
    }

    /// 直接按回退链取文本（纯字符串原样返回）。
    pub fn pick(&self, locale: &str) -> Option<&str> {
        match self {
            Self::Plain(s) => Some(s),
            Self::Localized(l) => l.pick(locale),
        }
    }
}

/// 作者信息（packaging.md §2.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// 来源分类（packaging.md §2.4）：只影响展示分组，不是信任边界。
/// 缺省一律 community——官方标识必须显式声明，防止第三方包伪造
/// "官方内置"徽章（安全默认）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginOrigin {
    #[default]
    Community,
    Official,
}

impl PluginOrigin {
    /// serde skip_serializing_if 谓词：community 不写进 JSON，v1 describe
    /// 字节保持不变。
    pub fn is_default(&self) -> bool {
        *self == Self::Community
    }
}

/// 插件元数据（§5.2 + packaging.md §2.2）。`version` 保持字符串（semver
/// 解析是宿主侧职责，不让 ABI 层依赖 semver 的错误形态）；`abi_version`
/// 必须等于 `PLUGIN_ABI_VERSION`，宿主校验不一致即拒绝加载（§5.7）。
///
/// v2 起的新字段只允许**追加在末尾**（冻结 ABI 纪律，AGENTS.md）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub abi_version: u32,
    pub kind: PluginKind,
    /// 每个工具自带 side_effect，不再有 manifest 级冗余字段（§5.2）。
    pub tools: Vec<ToolSpec>,

    // ---- v2 元数据（packaging.md §2.2，append-only）：缺失即优雅降级
    // （§7 Fallback 矩阵），不崩列表。 ----
    /// 展示名（可翻译；缺失时 UI 回退到 id）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<LocalizedString>,
    /// 描述（v1 字符串 / v2 多语言；缺失时不渲染描述区）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<LocalizedOrString>,
    /// 图标相对本 manifest 所在目录的路径（svg/png/webp；宿主经
    /// IconLoader 转 data URL，缺失时 UI 显示首字母方块）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Author>,
    #[serde(default, skip_serializing_if = "PluginOrigin::is_default")]
    pub origin: PluginOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// SPDX 标识（如 "MIT"）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest is not valid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("manifest json: {0}")]
    Json(#[from] serde_json::Error),
}

impl PluginManifest {
    /// 生成 describe 返回值：NUL 结尾的 UTF-8 JSON 字节串。
    ///
    /// JSON 文本不含裸 NUL（serde_json 会转义），`CString::new` 因此不会
    /// 失败；真失败说明 manifest 被塞了非法内容，panic 暴露问题即可。
    pub fn to_describe_json(&self) -> CString {
        let bytes = serde_json::to_vec(self).expect("manifest serializes to json");
        CString::new(bytes).expect("json text cannot contain a raw NUL")
    }

    /// 解析 describe 返回的字节串（不含结尾 NUL；宿主侧从 CStr 截取）。
    pub fn from_json(bytes: &[u8]) -> Result<Self, ManifestError> {
        let text = std::str::from_utf8(bytes)?;
        Ok(serde_json::from_str(text)?)
    }
}

#[cfg(test)]
mod tests;
