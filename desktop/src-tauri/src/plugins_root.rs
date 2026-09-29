//! plugins/ 安装目录解析与插件装载（布局 `plugins/{id}/{version}/`，§11.3）。
//!
//! 桌面端进程 CWD 不固定（tauri dev / 打包后各不同），所以从两个锚点
//! 逐级向上找：进程 CWD 与可执行文件目录；`LOCOWRK_PLUGINS_DIR` 优先。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use plugin_host::PluginRegistry;
use semver::Version;

const ANCESTOR_LIMIT: usize = 6;

/// 逐锚点逐级向上找 `plugins/`（判据：其下存在 tool-echo 子目录）。
pub fn resolve() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("LOCOWRK_PLUGINS_DIR") {
        let p = PathBuf::from(dir);
        if is_plugins_root(&p) {
            return Some(p);
        }
    }

    let cwd = std::env::current_dir().ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    for anchor in [cwd, exe_dir].into_iter().flatten() {
        let mut cur = Some(anchor);
        for _ in 0..ANCESTOR_LIMIT {
            let Some(p) = cur else { break };
            if is_plugins_root(&p.join("plugins")) {
                return Some(p.join("plugins"));
            }
            cur = p.parent().map(Path::to_path_buf);
        }
    }
    None
}

fn is_plugins_root(p: &Path) -> bool {
    p.join("tool-echo").is_dir()
}

/// 平台动态库文件名（AGENTS.md：Windows 下示例中的 `.so` 是 `.dll`）。
/// 插件 id 是 kebab-case，库产物用下划线：tool-echo → tool_echo.dll。
pub fn dll_name(id: &str) -> String {
    let stem = id.replace('-', "_");
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

/// 版本目录里的动态库发现（packaging.md §3.1："dll 名字由作者决定，
/// 宿主不假设文件名"）。约定名（`{id_underscored}.dll` 等）优先；没有
/// 就扫描目录里平台后缀的文件，恰好一个 → 采用，多个 → 视为歧义拒绝
/// （宁可漏载也不猜），零个 → None 由调用方记日志跳过。
pub fn find_artifact(ver_dir: &Path, id: &str) -> Option<PathBuf> {
    let conventional = ver_dir.join(dll_name(id));
    if conventional.is_file() {
        return Some(conventional);
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(ver_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
                    matches!(ext.to_ascii_lowercase().as_str(), "dll" | "so" | "dylib")
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
                    "multiple dynamic libraries in version dir, refusing to guess"
                );
            }
            None
        }
    }
}

/// 扫描目录下每个插件目录，取语义化版本最大的版本加载并激活。
/// 返回成功加载的 (id, version)；单个插件失败只记日志不中断其余。
pub fn load_all(
    root: &Path,
    registry: &Arc<PluginRegistry>,
) -> Result<Vec<(String, Version)>, String> {
    let mut loaded = Vec::new();
    let entries =
        std::fs::read_dir(root).map_err(|e| format!("read_dir {}: {e}", root.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((ver, ver_dir)) = latest_version(&path) else {
            continue;
        };
        let Some(dll) = find_artifact(&ver_dir, id) else {
            tracing::warn!(plugin = id, "artifact not found in {}", ver_dir.display());
            continue;
        };
        match registry
            .load(id, &ver, &dll)
            .and_then(|_| registry.activate(id, &ver))
        {
            Ok(_) => loaded.push((id.to_string(), ver)),
            Err(e) => tracing::warn!(plugin = id, "load/activate failed: {e}"),
        }
    }
    Ok(loaded)
}

/// 目录下语义化版本号最大的子目录。
fn latest_version(plugin_dir: &Path) -> Option<(Version, PathBuf)> {
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
