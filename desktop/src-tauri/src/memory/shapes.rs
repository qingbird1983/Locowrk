//! 记忆库的**形状与上限**（台账 T33 七片之首）：三个上限常量、管理页行形状
//! `MemoryRow`、两侧同源的权威建表串 `CREATE_TABLE`。
//! 这里只有形状，没有访问路径——读写在 `store.rs` / `crud.rs` / `reconcile.rs` /
//! `recall.rs`，改表形状（迁移）在 `schema.rs`。
//!
//! `LIST_MAX` 与 `CREATE_TABLE` 抬成 `pub(super)` 是本刀的机械后果：定义处仍然各只有
//! 一份（建表串被 `open_conn` 与 `ensure_workspace_schema` 共同读，那正是当初把它们
//! 收成一份的理由），可见范围收在 `memory` 子树内，模块外取不到。
//!
//! S1 刀②c-1 删掉了 `FactLine`（索引注入的指针行形状）：它唯一的消费者是"从库里现
//! 合成索引"那一档，那一档连同 `index_rows` 一起退役之后这个结构没人构造也没人读。

use serde::Serialize;

/// 与 memory-sqlite 插件同限：key 主键不能无限长，value 防 runaway 写。
pub(crate) const KEY_MAX: usize = 512;
pub(crate) const VALUE_MAX: usize = 256 * 1024;
/// 管理页单次列举上限（页内滚动不分页，200 条对"用户的笔记"足够）。
pub(super) const LIST_MAX: u32 = 200;

/// 管理页行（Tauri 返回值；snake_case 与既有 IPC 投影一致）。
#[derive(Debug, Clone, Serialize)]
pub struct MemoryRow {
    pub key: String,
    pub value: String,
    pub tags: String,
    /// "explicit" | "extracted"（前端按档位渲染人类文案，§8.1 结构化）。
    pub source: String,
    /// 项目归属（㉘）：会话关联的项目根，`""` = 未归属（§2.3 关联）。
    pub workspace: String,
    pub updated_at: i64,
}

/// 人类层读回的一条事实 → 行形状。**md 成真源之后，这一份映射就是唯一的装配点**：
/// 管理页列举（`crud::md_rows`）与召回注入（`recall::recall_once`）都走它。
/// 各写一份内联拷贝的话，字段一漂两侧表现就不一样（管理页看得见、模型看不见），
/// 而两边的单测会各自绿——这正是本仓"同一属性只许一个出处"那族判据的形状。
impl From<&plugin_api::memlayout::StoredFact> for MemoryRow {
    fn from(f: &plugin_api::memlayout::StoredFact) -> Self {
        Self {
            key: f.key.clone(),
            value: f.value.clone(),
            tags: f.tags.clone(),
            source: f.source.clone(),
            workspace: f.workspace.clone(),
            updated_at: f.updated_at,
        }
    }
}

/// 与 memory-sqlite 插件**逐字节同源**的权威表形状（两侧各持一份，迁移等价性依赖它
/// 一致；收尾判据 = `tmp/q5-parity.mjs` 剥注释后比常量本体与两条 ALTER 语句）。
/// ⚠ `IF NOT EXISTS` 是新加的：这一条串此前只服务 `ensure_workspace_schema` 的旧表重建，
/// `open_conn` 里另写了一份带 `IF NOT EXISTS` 的同义字面量 = **同一属性两个出处**（历批
/// 立的「统一与否要数还剩几个出处」在这里正好适用）⇒ 现在收成一份。
/// L1 学习面两列（ML §3.2 · 队列 Q5）：`hit_count` / `last_hit_at` 都是"学出来的"列，
/// 本批**没有任何读取点**（读它们做排序 = L2 / Q9），缺省 0 / NULL ⇒ 注入字节与召回集合
/// 逐字不变（等价判据 = 既有记忆面用例一字未改仍然绿）。
pub(super) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS memories (
    workspace  TEXT NOT NULL DEFAULT '',
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    tags       TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL,
    source     TEXT NOT NULL DEFAULT 'explicit',
    hit_count  INTEGER NOT NULL DEFAULT 0,
    last_hit_at INTEGER,
    PRIMARY KEY (workspace, key)
);";
