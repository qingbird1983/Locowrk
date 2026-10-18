//! memlayout 单测（自 565 行的 `tests.rs` 按面分簇拆出，**用例本体逐字未搬改**）。
//!
//! 拆的理由是 400 行纪律那条三问：这一份同时管着寻址、渲染、写删同步、索引预算、
//! 读回五件事，加一簇就牵动其余四簇共用的文件（先超线再挤）。共享夹具留本文件，
//! 用例按面分簇见下。
//!
//! 夹具纪律（本仓真踩过）：所有 md 正文一律经生产渲染 `render()` 生成，**不手写
//! md**——手写的夹具测不到渲染与解析那条往返，测试会全绿而真跑读回空字段。
//! 只有"旧格式文件"那两处必须手抄（现渲染已经多了一行 `tags:`，抄的就是它没有那行的形状），
//! 抄的本子 = `render()` 去掉那一行，不是凭想象。

mod index_budget;
mod layout;
mod read_back;
mod render_parse;
mod write_delete;

use super::*;

/// 一条最小事实（`tags` 与 `source` 走默认，要变的字段由用例自己改）。
/// updated_at 钉死一个常数：索引按 mtime 排序的分支要靠它保持稳定。
pub(super) fn entry<'a>(ws: &'a str, key: &'a str, value: &'a str) -> FactEntry<'a> {
    FactEntry {
        workspace: ws,
        key,
        value,
        tags: "",
        source: "explicit",
        updated_at: 1728100000,
    }
}

/// 每簇一个临时根（按 tag 与进程号命名，跑完各自清理）。
/// 用**当前进程 id** 而不是时间戳：同一秒内并发跑两条用例时，两个根不会互相踩。
pub(super) fn tmp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("memlayout-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}
