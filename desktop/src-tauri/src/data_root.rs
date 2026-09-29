//! 安装态数据根与市场目录解析（DESIGN.md §11.3 / §12.3）。
//!
//! 数据根（lockfile.json 与 plugins/ 的父目录）：`LOCOWRK_DATA_DIR` 优先，
//! 缺省 `%USERPROFILE%\.agent-kernel`（测试与便携场景用环境变量隔离）。
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
    Ok(PathBuf::from(home).join(".agent-kernel"))
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
