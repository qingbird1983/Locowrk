//! 队列 Q10（L4）起本文件承接 `memory_inject_log` 的**库形状**：建表 DDL 与 `mode` 列里
//! 注入档的取值。
//!
//! 为什么在这一刀拆出来：加一列要动的是"库长什么样"，而 `mod.rs` 其余部分是"怎么写、
//! 怎么读"（观测通路本体）。两者混在一处时，任何一次列变更都会把整个观测模块推过
//! 400 行——**拆簇是新增超限件的正解**（不抬基线、不压注释、不在文件间转字段），
//! 拆法照上批 `memory_inject/{mod,contract,terms}.rs` 的先例，正文逐字节搬移。
//!
//! ⚠ **S3 刀③b 摘掉的是"给存量库补 `breakdown` 列"的迁移件**（`ensure_breakdown_column`
//! 与 DDL 里那一列）：那一列记的是"那一次加权排序算出的四信号回执"，它的生产者
//! （`memory_learn/`）与唯一消费者（管理页的解释区块）同刀退役 ⇒ 新库不再建它、
//! 代码不再读写它。**存量库里那一列还在**（`CREATE TABLE IF NOT EXISTS` 不删列，
//! 本刀也不做 DROP COLUMN 迁移——那要动用户数据目录里的库，且这一列与
//! `memories.hit_count` / `last_hit_at` 是同一批影子遗物，去处统一随 S4 裁定）。
//! 登记口径见 `specs/10-context-injection-s3` 刀③b 施工记录。

/// 表 + 两个索引，每次开连接重跑（`IF NOT EXISTS` 幂等）——本模块因此不需要假设启动
/// 顺序，也不给 `usage.rs` 的建表函数加一个字。
///
/// `mode` = **召回的通道**，两个出处共用一行（Q1a 立表时就把这列留成了可扩展的取值面，
/// Q1b 因此零迁移）：`'inject'` = 注入侧每键位一行；`'memory_read'` / `'memory_search'` /
/// `'memory_project_history'` = 检索侧每次成功调用一行（不去重）。
///
/// `session_key` 用空串表示无会话（**不用 NULL**：部分唯一索引里 NULL ≠ NULL，用 NULL 会让
/// 无会话的注入每 step 插一条，去重直接失效）。
pub(crate) const DDL: &str = "
CREATE TABLE IF NOT EXISTS memory_inject_log (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    ts           INTEGER NOT NULL,
    mode         TEXT NOT NULL,
    session_key  TEXT NOT NULL,
    scope_id     TEXT NOT NULL,
    n_candidates INTEGER NOT NULL,
    returned     INTEGER NOT NULL,
    keys         TEXT NOT NULL,
    elapsed_us   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mil_mode_ts ON memory_inject_log(mode, ts);
CREATE UNIQUE INDEX IF NOT EXISTS ux_mil_inject ON memory_inject_log(session_key, scope_id) WHERE mode = 'inject';";

/// 注入通道在 `mode` 列里的取值（检索通道直接用工具名，与 `ToolSpec.name` 同串）。
/// SQL 里的这个串只有 DDL 的索引谓词是字面量（部分唯一索引的谓词不能参数化），
/// 写入与读取都绑这一个常量——同一读数两处写必漂，是本仓库登记过一次的教训。
pub(crate) const MODE_INJECT: &str = "inject";
