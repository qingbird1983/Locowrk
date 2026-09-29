//! 市场客户端（DESIGN.md §8 market.rs）：目录发现 + 包获取。
//!
//! [`Market`] 是抽象（未来可换 HTTP 实现）；[`LocalDirMarket`] 是阶段七的
//! 本地目录市场：`market/{id}/{version}/` 下放 `{id 下划线}.{dll|so|dylib}`
//! 与 manifest.json（packaging.md v2 全量 manifest，§3.1）。真实市场服务
//! 出现前，目录即市场。
//!
//! v2 纪律：manifest.json 按 `plugin_api::PluginManifest` 解析并交叉校验
//! id/version；坏 manifest 的版本目录直接跳过，不让整张市场列表崩
//! （packaging.md §11 常见坑"列表崩"）。

use std::path::{Path, PathBuf};

use plugin_api::manifest::PluginManifest;
use semver::Version;
use serde::Serialize;

use plugin_host::IconLoader;

use crate::display::{PluginDisplay, read_display};

/// 市场目录条目：一个插件的多版本清单（升序），last = latest；
/// 展示元数据取自 latest 版本目录的 manifest.json。
#[derive(Debug, Clone, Serialize)]
pub struct MarketplaceItem {
    pub id: String,
    pub versions: Vec<Version>,
    pub display: PluginDisplay,
}

impl MarketplaceItem {
    pub fn latest(&self) -> Option<&Version> {
        self.versions.last()
    }
}

/// 市场抽象：目录发现 + 取回一个具体版本的包（本地市场返回目录路径）。
pub trait Market: Send + Sync {
    fn catalog(&self) -> Result<Vec<MarketplaceItem>, String>;

    /// 校验并返回包目录（含 manifest.json 与平台库文件）。
    fn package(&self, id: &str, version: &Version) -> Result<PathBuf, String>;
}

/// 本地目录市场：root = `market/`，布局 `root/{id}/{version}/`。
pub struct LocalDirMarket {
    root: PathBuf,
    icons: IconLoader,
}

impl LocalDirMarket {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            icons: IconLoader::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 读并交叉校验一个版本的 manifest.json（packaging.md §3.1：包内
    /// manifest 与目录名必须自洽）。
    fn read_manifest(&self, id: &str, version: &Version) -> Result<PluginManifest, String> {
        let dir = self.package_dir(id, version);
        let path = dir.join("manifest.json");
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let m = PluginManifest::from_json(&bytes)
            .map_err(|e| format!("parse {}: {e}", path.display()))?;
        if m.id != id || m.version != version.to_string() {
            return Err(format!(
                "manifest mismatch in {}: declared {}@{}, expected {id}@{version}",
                dir.display(),
                m.id,
                m.version
            ));
        }
        Ok(m)
    }

    fn package_dir(&self, id: &str, version: &Version) -> PathBuf {
        self.root.join(id).join(version.to_string())
    }
}

impl Market for LocalDirMarket {
    fn catalog(&self) -> Result<Vec<MarketplaceItem>, String> {
        let mut items = Vec::new();
        // 市场目录还不存在（未跑构建脚本）→ 空目录，不报错
        let ids = match std::fs::read_dir(&self.root) {
            Ok(ids) => ids,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(items),
            Err(e) => return Err(format!("read_dir {}: {e}", self.root.display())),
        };
        for id_entry in ids.flatten() {
            let id_dir = id_entry.path();
            if !id_dir.is_dir() {
                continue;
            }
            let Some(id) = id_dir.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let mut versions = Vec::new();
            if let Ok(vers) = std::fs::read_dir(&id_dir) {
                for v_entry in vers.flatten() {
                    let v_dir = v_entry.path();
                    if !v_dir.is_dir() {
                        continue;
                    }
                    let Some(name) = v_dir.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    let Ok(v) = Version::parse(name) else {
                        continue;
                    };
                    // 只列出 artifact 齐全、manifest 可读且自洽的版本
                    if !v_dir.join(artifact_file_name(id)).exists() {
                        continue;
                    }
                    if self.read_manifest(id, &v).is_err() {
                        continue;
                    }
                    versions.push(v);
                }
            }
            versions.sort();
            if !versions.is_empty() {
                // 卡片展示取 latest 版本的 manifest（市场永远展示最新形象）
                let display =
                    read_display(&self.package_dir(id, versions.last().unwrap()), &self.icons);
                items.push(MarketplaceItem {
                    id: id.to_string(),
                    versions,
                    display,
                });
            }
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(items)
    }

    fn package(&self, id: &str, version: &Version) -> Result<PathBuf, String> {
        let dir = self.package_dir(id, version);
        if !dir.is_dir() {
            return Err(format!("package {id}@{version} not in market"));
        }
        if !dir.join(artifact_file_name(id)).exists() {
            return Err(format!(
                "package {id}@{version}: artifact {} missing",
                artifact_file_name(id)
            ));
        }
        self.read_manifest(id, version)?;
        Ok(dir)
    }
}

/// 平台库文件名（AGENTS.md：Windows 下设计示例的 `.so` 是 `.dll`）。
/// 插件 id 是 kebab-case，库产物用下划线：tool-shout → tool_shout.dll。
pub fn artifact_file_name(id: &str) -> String {
    let stem = id.replace('-', "_");
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

#[cfg(test)]
mod tests;
