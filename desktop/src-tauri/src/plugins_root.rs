//! plugins/ 安装目录解析与插件装载（布局 `plugins/{id}/{version}/`，§11.3）。
//!
//! 桌面端进程 CWD 不固定（tauri dev / 打包后各不同），所以从两个锚点
//! 逐级向上找：进程 CWD 与可执行文件目录；`LOCOWRK_PLUGINS_DIR` 优先。
//! S2 起：装载尊重 lockfile——跳过 `PluginService::dev_scan_skip_ids`
//! 给出的 id（安装态接管 + 已停用的内置插件）；版本/工件解析下沉到
//! `plugin_service::builtin` 共享（服务层重新启用内置插件也要用）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use plugin_host::PluginRegistry;
use semver::Version;
// lib 本体只用 find_artifact；dll_name 由 tests_plugins_root.rs 经此路径
// 引用（保持测试里 `plugins_root::dll_name` 的既有写法）。
#[allow(unused_imports)]
pub use plugin_service::builtin::{dll_name, find_artifact};

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

/// 扫描目录下每个插件目录，取语义化版本最大的版本加载并激活。
/// `skip` 里的 id 直接跳过（S2：lockfile 已接管的安装条目 + 用户停用的
/// 内置插件）。返回成功加载的 (id, version)；单个插件失败只记日志不中断。
pub fn load_all(
    root: &Path,
    registry: &Arc<PluginRegistry>,
    skip: &HashSet<String>,
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
        if skip.contains(id) {
            tracing::info!(
                plugin = id,
                "dev scan skipped (lockfile managed or disabled)"
            );
            continue;
        }
        let Some((ver, ver_dir)) = plugin_service::builtin::resolve_latest(&path) else {
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
