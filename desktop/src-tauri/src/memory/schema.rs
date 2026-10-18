//! **旧库迁移**（台账 T33）：`ensure_workspace_schema`（key 单主键时代整表重建进
//! 未归属键位）与 `ensure_learning_schema`（L1 补 `hit_count` / `last_hit_at`，逐列决定
//! 补哪条 ⇒ 半迁状态可自愈）。
//!
//! 两个函数与插件侧 `plugins-src/memory-sqlite/src/store.rs` 同源，两侧对账那道闸
//! （`scripts/check-schema-parity.mjs`）从 T23 起按**整根**定位定义处，所以搬到本片
//! 不需要改闸；`pub(super)` 前缀也不影响它的取段锚——锚从 `fn ensure_learning_schema(`
//! 起算，前面那几个字节本来就不进比对。建表串在 `shapes.rs`，这里经 `super::shapes::`
//! 读它，不抄第二份。

use rusqlite::{params, Connection};

use super::shapes::CREATE_TABLE;

/// 旧库迁移：无 workspace 列 = key 单主键时代，整表重建进未归属键位。
/// 与插件侧（memory-sqlite store.rs）等价幂等：BEGIN IMMEDIATE + 二次
/// 探针，两侧并发只有一方真正执行；旧表 source 列可有可无（headless
/// 时代插件建的库没有）——探针决定 SELECT 中的 source 表达式。
pub(super) fn ensure_workspace_schema(conn: &Connection) -> Result<(), String> {
    let has_col = |name: &str| -> Result<i64, String> {
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('memories') WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .map_err(|e| format!("memory db schema probe: {e}"))
    };
    if has_col("workspace")? > 0 {
        return Ok(());
    }
    let source_expr = if has_col("source")? > 0 {
        "source"
    } else {
        "'explicit'"
    };
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| format!("memory db migrate begin: {e}"))?;
    let migrate = || -> Result<(), String> {
        if has_col("workspace")? > 0 {
            return Ok(());
        }
        conn.execute_batch(&format!(
            "ALTER TABLE memories RENAME TO memories_legacy_ws;
             {CREATE_TABLE}
             INSERT INTO memories (workspace, key, value, tags, updated_at, source)
                 SELECT '', key, value, tags, updated_at, {source_expr} FROM memories_legacy_ws;
             DROP TABLE memories_legacy_ws;"
        ))
        .map_err(|e| format!("memory db migrate to workspace key: {e}"))
    };
    match migrate() {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|e| format!("memory db migrate commit: {e}")),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}

/// L1 学习面迁移（ML §3.2 · 后审计队列 Q5）：旧库补 `hit_count` / `last_hit_at` 两列。
/// 形状照上面的 `ensure_workspace_schema`（探针 + `BEGIN IMMEDIATE` + 等锁后二次探针），
/// 理由是宿主与插件开的是**同一个** `memory.sqlite3`——两侧并发时只有一方真正执行 ALTER。
/// ⚠ **逐列**决定补哪一条，而不是「缺一就两条都跑」：两条 ALTER 在同一批次里顺序执行，
/// 中间被杀（进程崩 / 掉电）会留下「有 `hit_count`、无 `last_hit_at`」的半迁状态，那时
/// 重跑第一条会撞 `duplicate column name` ⇒ Err → ROLLBACK ⇒ 这个库**永远打不开**（缺陷
/// 形状比漏列更坏）。现写法幂等且**可自愈**：缺哪条补哪条，两条都在直接返回。
/// 迁移不可逆（ML §5.2：drop 列会连带丢学习数据）——回滚说明按稿注在 DESIGN.md 侧。
pub(super) fn ensure_learning_schema(conn: &Connection) -> Result<(), String> {
    let has_col = |name: &str| -> Result<i64, String> {
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('memories') WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .map_err(|e| format!("memory db schema probe: {e}"))
    };
    let missing_sql = || -> Result<String, String> {
        let mut sql = String::new();
        if has_col("hit_count")? == 0 {
            sql.push_str("ALTER TABLE memories ADD COLUMN hit_count INTEGER NOT NULL DEFAULT 0;\n");
        }
        if has_col("last_hit_at")? == 0 {
            sql.push_str("ALTER TABLE memories ADD COLUMN last_hit_at INTEGER;");
        }
        Ok(sql)
    };
    if missing_sql()?.is_empty() {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| format!("memory db migrate begin: {e}"))?;
    let migrate = || -> Result<(), String> {
        // 并发方可能已在我们等锁期间迁完（竞态全靠这第二次探针，与上面同形）
        let sql = missing_sql()?;
        if sql.is_empty() {
            return Ok(());
        }
        conn.execute_batch(&sql)
            .map_err(|e| format!("memory db migrate to learning columns: {e}"))
    };
    match migrate() {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|e| format!("memory db migrate commit: {e}")),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}
