//! 安装态数据根与市场目录解析（DESIGN.md §11.3 / §12.3）。
//!
//! 数据根（lockfile.json 与 plugins/ 的父目录）：`LOCOWRK_DATA_DIR` 优先，
//! 缺省 `%USERPROFILE%\.Locowrk`（测试与便携场景用环境变量隔离）。
//! 市场目录：`LOCOWRK_MARKET_DIR` 优先，否则从 CWD / exe 目录逐级向上找
//! `market/`（判据：其下存在 tool-shout 子目录，同 plugins_root 的启发式）。

use std::path::{Path, PathBuf};

const ANCESTOR_LIMIT: usize = 6;

pub fn resolve_data_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("LOCOWRK_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "USERPROFILE / HOME 均未设置，无法定位安装态数据根")?;
    Ok(PathBuf::from(home).join(".Locowrk"))
}

/// 逐锚点逐级向上找 `market/`；找不到返回 None（市场页显示为空目录）。
pub fn resolve_market_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("LOCOWRK_MARKET_DIR") {
        let p = PathBuf::from(dir);
        if is_market_root(&p) {
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
            if is_market_root(&p.join("market")) {
                return Some(p.join("market"));
            }
            cur = p.parent().map(Path::to_path_buf);
        }
    }
    None
}

fn is_market_root(p: &Path) -> bool {
    p.join("tool-shout").is_dir()
}

/// 数据根解析（state/init_state 调用）：LOCOWRK_DATA_DIR 优先，失败退
/// 系统临时目录（组装点先警告再降级，聊天不受影响）。
pub fn data_dir() -> std::path::PathBuf {
    resolve_data_dir().unwrap_or_else(|e| {
        tracing::warn!("{e}；退回系统临时目录");
        std::env::temp_dir().join("locowrk-data")
    })
}

/// ⑱ fs roots env 注入（settings `tools.fs_roots` → `LOCOWRK_FS_ROOTS`，
/// `;` 分隔同 fsutil 解析约定）：空表 = 移除 env（回落缺省只放行数据根）。
/// init_state 与 settings_set（tools 区有改动时）各调一次。
pub fn apply_fs_roots_env(settings: &crate::settings::SettingsStore) {
    let roots = settings.fs_roots();
    if roots.is_empty() {
        std::env::remove_var("LOCOWRK_FS_ROOTS");
    } else {
        std::env::set_var("LOCOWRK_FS_ROOTS", roots.join(";"));
    }
}
