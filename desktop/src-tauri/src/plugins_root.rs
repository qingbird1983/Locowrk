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
        let dll = ver_dir.join(dll_name(id));
        if !dll.exists() {
            tracing::warn!(plugin = id, "artifact missing: {}", dll.display());
            continue;
        }
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
