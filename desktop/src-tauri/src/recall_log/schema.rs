//! 队列 Q10（L4）起本文件承接 `memory_inject_log` 的**库形状**：建表 DDL、`mode` 列里
//! 注入档的取值，以及给存量库补 `breakdown` 列的幂等迁移。
//!
//! 为什么在这一刀拆出来：加一列要动的是"库长什么样"，而 `mod.rs` 其余部分是"怎么写、
//! 怎么读"（观测通路本体）。两者混在一处时，任何一次列变更都会把整个观测模块推过
//! 400 行——**拆簇是新增超限件的正解**（不抬基线、不压注释、不在文件间转字段），
//! 拆法照上批 `memory_inject/{mod,contract,terms}.rs` 的先例，正文逐字节搬移。

use rusqlite::{params, Connection};

/// 表 + 两个索引，每次开连接重跑（`IF NOT EXISTS` 幂等）——本模块因此不需要假设启动
/// 顺序，也不给 `usage.rs` 的建表函数加一个字。
///
/// `mode` = **召回的通道**，两个出处共用一行（Q1a 立表时就把这列留成了可扩展的取值面，
/// Q1b 因此零迁移）：`'inject'` = 注入侧每键位一行；`'memory_read'` / `'memory_search'` /
/// `'memory_project_history'` = 检索侧每次成功调用一行（不去重）。
///
/// `session_key` 用空串表示无会话（**不用 NULL**：部分唯一索引里 NULL ≠ NULL，用 NULL 会让
/// 无会话的注入每 step 插一条，去重直接失效）。
///
/// ⚠ **`breakdown` 是可空的**（队列 Q10 / ML 稿 §3.1 原则 4）：它记的是"那一次加权排序
/// 算出来的四信号回执"。三档情形都必须能被区分，所以不给 `NOT NULL DEFAULT ''`——
/// ① 检索档（工具回执）根本没有分数这一步 → NULL；② 注入档但学习档关着（现役 SQL 召回）
/// → NULL；③ 注入档且开着学习档 → JSON。空串会把 ①② 读成"有回执但回执为空"，
/// 那是假数据（与本模块 `store_empty_ratio` 用 `None` 不用 `0.0` 同一条口径）。
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
    elapsed_us   INTEGER NOT NULL,
    breakdown    TEXT
);
CREATE INDEX IF NOT EXISTS idx_mil_mode_ts ON memory_inject_log(mode, ts);
CREATE UNIQUE INDEX IF NOT EXISTS ux_mil_inject ON memory_inject_log(session_key, scope_id) WHERE mode = 'inject';";

/// 注入通道在 `mode` 列里的取值（检索通道直接用工具名，与 `ToolSpec.name` 同串）。
/// SQL 里的这个串只有 DDL 的索引谓词是字面量（部分唯一索引的谓词不能参数化），
/// 写入与读取都绑这一个常量——同一读数两处写必漂，是本仓库登记过一次的教训。
pub(crate) const MODE_INJECT: &str = "inject";

/// 给**存量库**补 `breakdown` 列（Q10/L4 唯一的磁盘动作，可空列 ⇒ 不改写任何既有行）。
///
/// 为什么 `CREATE TABLE IF NOT EXISTS` 不够：那句对**已存在的表不补列**，本机
/// `usage.sqlite3` 里的 `memory_inject_log` 早就在场（实测有行数），于是新列只能靠
/// 显式 `ALTER`。形状逐字照 `memory.rs::ensure_learning_schema`（探针 → `BEGIN IMMEDIATE`
/// → 等锁后二次探针 → 补列 → COMMIT / ROLLBACK）——同一件事在本仓只该有一套写法。
///
/// 幂等且可自愈：列已在场直接返回，不启动事务、不写盘；中途被杀后重跑走同一支路。
/// 二次探针不是冗余：并发方（另一个组装点）可能在我们等锁期间已经补完，那时
/// 再 `ALTER` 会撞 `duplicate column name` ⇒ Err → ROLLBACK ⇒ 这个库**永远打不开**
/// （Q5 登记过的缺陷形状，比漏列更坏）。
pub(crate) fn ensure_breakdown_column(conn: &Connection) -> Result<(), String> {
    let has_col = || -> Result<i64, String> {
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('memory_inject_log') WHERE name = ?1",
            params!["breakdown"],
            |r| r.get(0),
        )
        .map_err(|e| format!("recall log schema probe: {e}"))
    };
    if has_col()? > 0 {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| format!("recall log migrate begin: {e}"))?;
    let migrate = || -> Result<(), String> {
        // 等锁期间的竞态全靠这第二次探针（与上面同形）
        if has_col()? > 0 {
            return Ok(());
        }
        conn.execute_batch("ALTER TABLE memory_inject_log ADD COLUMN breakdown;")
            .map_err(|e| format!("recall log migrate to breakdown: {e}"))
    };
    match migrate() {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|e| format!("recall log migrate commit: {e}")),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}
