//! memory 存储单测分簇（v2.189 · 队列 Q5）：helper 与夹具留本文件，用例按面分簇——
//! `legacy` = 既有形状（CRUD / 过滤 / 上限校验 / 旧库 workspace 迁移 / 项目键位隔离 ㉘ /
//! MemorySource 投影），**用例本体逐字节搬移**；`learning` = L1 学习面两列（ML §3.2）：
//! 那两列的**建表与迁移形状**（S3 刀③b 之后它们没人读写，列本身留到 S4 一起裁定）；
//! **已退役的簇**：`recall_learn` = L2+L3 生产路径（加权排序 + 写回 + 对照档），随
//! `memory_learn/` 整族在同一天删除（S3 刀③b · 裁定 = specs/10 开工三问②）；
//! `md_read` = 刀②b 起的**读源**（管理页列举读 md、不读库，含"把库改名也照常出数"那条）；
//! `inject_row` = 刀③b-2 从 `md_read` 拆出的**注入记账**那一族（降级照常交付 + 埋点宁缺毋伪）；
//! `layer_switches` = 刀④b 的**两枚行为开关**（ADR-0007 §18）：未接线 = 恒开、关档只出不进；
//! `index` = Q2 项目记忆**索引注入**（整节缺席的条件 / 指针行透传 / 陈旧标注现算）；
//! `layers` = 刀④c 的**每层一行与回收区**（按目录给数、软删与真删分档、hint 对不上时报什么）。
//! 拆簇理由 = 三问自检第①问：加完学习面判据这个文件要同时管「存储面既有形状」与
//! 「schema 迁移与学习列」两件事，且它已 370 行、必然越过 400 行闸（不抬基线、不压注释）。

mod index;
mod inject_row;
mod layer_switches;
mod layers;
mod learning;
mod legacy;
mod md_read;

use super::MemoryStore;
use kernel_agent::MemorySource;
use std::path::PathBuf;
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    crate::testutil::unique_dir(&format!("memstore-{}", tag))
}

/// 人类层接线夹具。**刀②c-1 起它没有第二个参数了**：真源 = md，`MemoryStore` 自己
/// 按数据根算出句柄，不存在"未接线的 store"这一档（原先那个 `on: bool` 传的是
/// settings `memory.files`，它现在管不到宿主读写面）。名字留着只因为它仍然表达
/// "这是一个连好真源的 store"——而它的反面已经没有实现可供构造。
fn wired(dir: &std::path::Path) -> MemoryStore {
    MemoryStore::new(dir, test_resolver())
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
