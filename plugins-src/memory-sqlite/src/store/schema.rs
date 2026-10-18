//! **表形状与开库路径**（台账 T35）：权威建表串 `CREATE_TABLE`、两条旧库迁移
//! （`ensure_learning_schema` 补 L1 两列、`ensure_workspace_schema` 把 key 单主键时代的整表
//! 重建进未归属键位）与 `open()`（开连接 + busy_timeout + 建表 + 依序跑两条迁移）。
//!
//! 四件合一片的理由不是省事：`open()` 夹在两条迁移中间，读的就是"把库带到 CREATE_TABLE
//! 声明的那个形状"这一条线——拆开就要在两个文件里各读一遍同一套 `BEGIN IMMEDIATE` +
//! 二次探针纪律。与宿主 `desktop/src-tauri/src/memory/{shapes,schema}.rs` 同源的那两份
//! 对账由 `scripts/check-schema-parity.mjs` 按**整根**定位（T23 起），搬到哪里都跟得到。
//! ⚠ 建表串在这里保持**私有**（宿主那侧因跨片共读抬成了 `pub(super)`）：对账闸的取段锚落在这两条
//! 定义的**名字本身**上，前缀本来就不进比对 ⇒ 两种形状同时在场，正好把"闸对可见性前缀不敏感"这条
//! 实测过第二遍。（闸的锚串**不许抄进头注**——抄了就会让打对照的人改到注释上，台账 T35 记的坑。）

use std::time::Duration;

use rusqlite::{Connection, params};
/// 当前权威表形状（两侧同源；`source` 列归宿主演进，缺省 explicit）。
const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS memories (
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

/// L1 学习面迁移（ML §3.2 · 后审计队列 Q5）：旧库补 `hit_count` / `last_hit_at` 两列。
/// 形状照上面的 `ensure_workspace_schema`（探针 + `BEGIN IMMEDIATE` + 等锁后二次探针），
/// 理由是宿主与插件开的是**同一个** `memory.sqlite3`——两侧并发时只有一方真正执行 ALTER。
/// ⚠ **逐列**决定补哪一条，而不是「缺一就两条都跑」：两条 ALTER 在同一批次里顺序执行，
/// 中间被杀（进程崩 / 掉电）会留下「有 `hit_count`、无 `last_hit_at`」的半迁状态，那时
/// 重跑第一条会撞 `duplicate column name` ⇒ Err → ROLLBACK ⇒ 这个库**永远打不开**（缺陷
/// 形状比漏列更坏）。现写法幂等且**可自愈**：缺哪条补哪条，两条都在直接返回。
/// 迁移不可逆（ML §5.2：drop 列会连带丢学习数据）——回滚说明按稿注在 DESIGN.md 侧。
fn ensure_learning_schema(conn: &Connection) -> Result<(), String> {
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

pub(super) fn open() -> Result<Connection, String> {
    let dir = plugin_api::fsutil::data_root()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("memory db dir `{}`: {e}", dir.display()))?;
    let conn =
        Connection::open(dir.join("memory.sqlite3")).map_err(|e| format!("memory db open: {e}"))?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|e| format!("memory db busy_timeout: {e}"))?;
    // 表形状走与宿主逐字节同源的 CREATE_TABLE（此前这里另写一份同义字面量 = 同一属性
    // 两个出处，队列 Q5 收成一份；parity 由 tmp/q5-parity.mjs 盯着）。
    conn.execute_batch(CREATE_TABLE)
        .map_err(|e| format!("memory db init: {e}"))?;
    ensure_workspace_schema(&conn)?;
    ensure_learning_schema(&conn)?;
    Ok(conn)
}

/// 旧库迁移：无 workspace 列 = key 单主键时代，整表重建进新键位
/// （旧行全部归入未归属 `''`）。幂等 + BEGIN IMMEDIATE 内二次探针，
/// 与宿主侧迁移并发时只有一方真正执行。
fn ensure_workspace_schema(conn: &Connection) -> Result<(), String> {
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
        // 并发方可能已在我们等锁期间迁完
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
