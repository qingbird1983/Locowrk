//! memory 存储单测分簇（v2.189 · 队列 Q5）：helper 与夹具留本文件，用例按面分簇——
//! `legacy` = 既有形状（CRUD / 过滤 / 上限校验 / 旧库 workspace 迁移 / 项目键位隔离 ㉘ /
//! MemorySource 投影），**用例本体逐字节搬移**；`learning` = L1 学习面两列（ML §3.2）；
//! `recall_learn` = L2+L3 生产路径（ML §3.3/§3.4 · 队列 Q9：加权排序 + 写回 + 对照档）；
//! `md_read` = 刀②b 起的**读源**（管理页列举读 md、不读库，含"把库改名也照常出数"那条）。
//! 拆簇理由 = 三问自检第①问：加完学习面判据这个文件要同时管「存储面既有形状」与
//! 「schema 迁移与学习列」两件事，且它已 370 行、必然越过 400 行闸（不抬基线、不压注释）。

mod learning;
mod legacy;
mod md_read;
mod recall_learn;

use super::MemoryStore;
use kernel_agent::MemorySource;
use std::path::PathBuf;
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    crate::testutil::unique_dir(&format!("memstore-{}", tag))
}

/// 人类层接线夹具：`on` = 总开关（settings `memory.files`）。刀②b 起有两个簇共用
/// （`legacy` 测双写、`md_read` 测读源），故从 `legacy.rs` 搬进本文件——
/// 定义处仍只有一份。
fn wired(dir: &std::path::Path, on: bool) -> MemoryStore {
    let files = Arc::new(crate::memory_files::MemoryFiles::new(dir));
    let flag: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(move || on);
    MemoryStore::new(dir, test_resolver()).with_files(files, flag)
}

/// 归一键位（Q2 起 `scope_of_session` 出来的串才是查行的键位）。测试用同一个
/// helper 取键位，跨平台两侧一致（Windows 上它是 `f:\ws\a`，POSIX 上仍是原串）。
fn keysid() -> String {
    crate::workspace::scope_key(Some("F:/ws/a"))
}

/// 测试解析器：`ws-session` → 项目根，其余未归属。
///
/// `ws-variant-session` 给的是**同一个项目的另一种写法**（斜杠方向 + 尾分隔符），
/// 专供 Q2（MR §6）的键位归一判据对——两个会话 id 必须落进同一个键位。
fn test_resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|sid: &str| match sid {
        "ws-session" => Some("F:/ws/a".to_string()),
        "ws-variant-session" => Some(r"F:\ws\a\".to_string()),
        _ => None,
    })
}
