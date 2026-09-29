//! 插件元数据契约（DESIGN.md §5.2）。JSON 是传输形态：describe 符号返回
//! NUL 结尾 UTF-8 JSON（长度协议见 `ffi::DescribeFn`），安装目录里的
//! manifest.json（开发态布局见 plugins/README.md）读出的也是同一份结构。

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

/// 插件元数据（§5.2）。`version` 保持字符串（semver 解析是宿主侧职责，
/// 不让 ABI 层依赖 semver 的错误形态）；`abi_version` 必须等于
/// `PLUGIN_ABI_VERSION`，宿主校验不一致即拒绝加载（§5.7）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub abi_version: u32,
    pub kind: PluginKind,
    /// 每个工具自带 side_effect，不再有 manifest 级冗余字段（§5.2）。
    pub tools: Vec<ToolSpec>,
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
