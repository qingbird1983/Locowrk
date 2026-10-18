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
mod kind;
mod layout;
mod read_back;
mod render_parse;
mod write_delete;

use super::paths;
use super::*;

/// 拆进 `paths.rs`（S1 刀②c-1）之后由测试**直接点名**取的两件：开关语义的纯函数判据
/// `enabled_from`、层级目录常量 `GLOBAL_DIR`（夹具里算落点用）。它们不经 `mod.rs`
/// 再导出——生产代码不引用它们，挂在 `mod.rs` 上就是一条 `unused imports` warning，
/// 而本仓 clippy 是 `-D warnings`：为了消除 warning 而把测试要的东西塞进公开面，
/// 那是让闸的形状决定 API 的形状，方向反了。
pub(crate) use paths::{GLOBAL_DIR, enabled_from};

/// 一条最小事实（`tags` 与 `source` 走默认，要变的字段由用例自己改）。
/// updated_at 钉死一个常数：索引按 mtime 排序的分支要靠它保持稳定。
///
/// `ty` 走**未分类**（空串）：那正是四类型落地之前每个文件的形状，
/// 所以 `render_parse` / `write_delete` 那些既有断言（含手抄的旧 frontmatter 本子）
/// 一字不改仍然绿——这条担保由 `kind.rs::an_entry_without_a_type_renders_the_old_bytes` 直说。
pub(super) fn entry<'a>(ws: &'a str, key: &'a str, value: &'a str) -> FactEntry<'a> {
    FactEntry {
        workspace: ws,
        key,
        value,
        tags: "",
        source: "explicit",
        updated_at: 1728100000,
        ty: "",
    }
}

/// 每簇一个临时根（按 tag 与进程号命名，跑完各自清理）。
/// 用**当前进程 id** 而不是时间戳：同一秒内并发跑两条用例时，两个根不会互相踩。
pub(super) fn tmp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("memlayout-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}
