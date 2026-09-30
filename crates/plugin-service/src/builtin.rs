//! 内置插件（开发态 `plugins/` 目录直载）的版本与工件解析（S2，清单 §1）。
//!
//! 内置插件随程序分发、不经市场安装，但停用状态要落 lockfile 持久化，
//! 重新启用时服务层必须能自己找到加载工件——原来这套解析住在桌面端
//! `plugins_root.rs`，现在服务层也要用，就地下沉到本模块共享
//! （`pub` 导出，桌面端复用，避免两份实现漂移）。

use std::path::{Path, PathBuf};

use semver::Version;

/// 平台动态库约定名（与 [`crate::market::artifact_file_name`] 同一套规则，
/// 语义上分属"开发态目录布局"与"市场包布局"，各自调用）。
pub fn dll_name(id: &str) -> String {
    crate::market::artifact_file_name(id)
}

/// 版本目录里的加载工件发现（packaging.md §3.1："dll 名字由作者决定，
/// 宿主不假设文件名"；v2.16 起目录里也可以是 `.wasm`）。约定 native 名
/// 优先，其次约定 wasm 名（`{id_underscored}.wasm`，跨平台单名无 lib
/// 前缀）；都没有就扫描目录里平台后缀的文件，恰好一个 → 采用，
/// 多个 → 视为歧义拒绝（宁可漏载也不猜），零个 → None 由调用方记日志。
pub fn find_artifact(ver_dir: &Path, id: &str) -> Option<PathBuf> {
    let conventional = ver_dir.join(dll_name(id));
    if conventional.is_file() {
        return Some(conventional);
    }
    let stem = id.replace('-', "_");
    let wasm_conventional = ver_dir.join(format!("{stem}.wasm"));
    if wasm_conventional.is_file() {
        return Some(wasm_conventional);
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(ver_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "dll" | "so" | "dylib" | "wasm"
                    )
                })
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [only] => Some(only.clone()),
        many => {
            if many.len() > 1 {
                tracing::warn!(
                    plugin = id,
                    dir = %ver_dir.display(),
                    "multiple loadable artifacts (dynlib/wasm) in version dir, refusing to guess"
                );
            }
            None
        }
    }
}

/// 插件目录下语义化版本号最大的子目录（内置插件独立升级，启用时取最新）。
pub fn resolve_latest(plugin_dir: &Path) -> Option<(Version, PathBuf)> {
    let entries = std::fs::read_dir(plugin_dir).ok()?;
    let mut best: Option<(Version, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Ok(v) = Version::parse(name) {
            if best.as_ref().is_none_or(|(b, _)| v > *b) {
                best = Some((v, path));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests;
