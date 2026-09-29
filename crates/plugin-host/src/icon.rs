//! 图标加载（packaging.md §5.2）：把插件目录里的图标文件读成
//! `data:<mime>;base64,…` data URL，前端 `<img src>` 直接可用，
//! 不需要感知文件系统细节。
//!
//! 安全与性能纪律（§4 / §11 常见坑）：
//! - 路径穿越防护：icon 相对路径 canonicalize 后必须仍位于插件目录内
//! - 大小上限 1MB，超限拒绝（防大图卡 UI）
//! - 只认 svg / png / webp（§4.1：GIF 动图、JPEG 伪影、ICO 平台专属不支持）
//! - 结果缓存（含 None 的负缓存）：plugin_list 每次刷新都调，不能每次读盘
//!
//! 任何失败都返回 None 并只记 warn——图标缺失走 UI 首字母兜底，
//! 绝不让列表崩（§7 Fallback 矩阵）。

use std::path::{Path, PathBuf};

use base64::Engine as _;

/// 单图标上限（packaging.md §11：图片几 MB 直接 base64 会卡 UI）。
const MAX_BYTES: usize = 1024 * 1024;

/// 已加载图标的缓存（含 None 的负缓存）。key = (插件目录, 相对路径)。
pub struct IconLoader {
    cache: dashmap::DashMap<(PathBuf, String), Option<String>>,
}

impl IconLoader {
    pub fn new() -> Self {
        Self {
            cache: dashmap::DashMap::new(),
        }
    }

    /// 读取 `plugin_dir/icon_rel` 并编码为 data URL。失败返回 None
    /// （穿越 / 超限 / 格式不支持 / 文件不存在）。
    pub fn load(&self, plugin_dir: &Path, icon_rel: &str) -> Option<String> {
        let key = (plugin_dir.to_path_buf(), icon_rel.to_owned());
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let result = self.load_inner(plugin_dir, icon_rel);
        self.cache.insert(key, result.clone());
        result
    }

    fn load_inner(&self, plugin_dir: &Path, icon_rel: &str) -> Option<String> {
        // 防路径穿越：canonicalize 后必须仍在插件目录内（§11 常见坑：
        // icon: "../../etc/passwd"）
        let canon_dir = plugin_dir.canonicalize().ok()?;
        let canon_path = plugin_dir.join(icon_rel).canonicalize().ok()?;
        if !canon_path.starts_with(&canon_dir) {
            tracing::warn!(dir = %plugin_dir.display(), rel = icon_rel, "icon path escapes plugin dir, rejected");
            return None;
        }
        let bytes = std::fs::read(&canon_path).ok()?;
        if bytes.len() > MAX_BYTES {
            tracing::warn!(path = %canon_path.display(), "icon too large, rejected");
            return None;
        }
        let mime = match canon_path.extension().and_then(|e| e.to_str()) {
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            _ => {
                tracing::warn!(path = %canon_path.display(), "unsupported icon format");
                return None;
            }
        };
        Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }
}

impl Default for IconLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for IconLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IconLoader")
            .field("cached", &self.cache.len())
            .finish()
    }
}

#[cfg(test)]
mod tests;
