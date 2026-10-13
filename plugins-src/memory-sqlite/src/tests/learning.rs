//! L1 学习面两列在**插件侧**的形状（ML §3.2 · 后审计队列 Q5）。
//!
//! ⚠ 宿主（`desktop/src-tauri/src/memory/tests/learning.rs`）与这一簇判的是同一张表的同一
//! 组性质，但**必须两侧各写一遍**：宿主与插件开的是同一个 `memory.sqlite3`，谁先访问谁
//! 执行迁移，另一侧只是"探针发现已迁完"。只在一侧有用例 = 另一侧的迁移分支无人走过
//! （两侧的 `ensure_learning_schema` 是两份源码，parity 脚本只保证**语句**相同，不保证
//! 两边都能跑通——跑通这件事只能各测各的）。

use super::*;

use rusqlite::Connection;

fn cols(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info('memories')")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn learning(conn: &Connection, key: &str) -> (i64, Option<i64>) {
    conn.query_row(
        "SELECT hit_count, last_hit_at FROM memories WHERE key = ?1",
        [key],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// 造一个**没有学习列**的旧形状库（`extra` 给半迁夹具追加列）。
fn make_db(db: &std::path::Path, extra: &str) {
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    let conn = Connection::open(db).unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE memories (
             workspace  TEXT NOT NULL DEFAULT '',
             key        TEXT NOT NULL,
             value      TEXT NOT NULL,
             tags       TEXT NOT NULL DEFAULT '',
             updated_at INTEGER NOT NULL,
             source     TEXT NOT NULL DEFAULT 'explicit',{extra}
             PRIMARY KEY (workspace, key)
         );
         INSERT INTO memories (workspace, key, value, tags, updated_at, source)
             VALUES ('', 'old.k', 'old-value', 't', 123, 'explicit');"
    ))
    .unwrap();
}

/// 旧库首次访问补两列、原行不动、再访问幂等。
#[test]
fn legacy_db_gains_learning_columns_without_losing_rows() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("learn-migrate");
    let db = dir.join("memory.sqlite3");
    make_db(&db, "");
    let conn = Connection::open(&db).unwrap();
    assert_eq!(cols(&conn).len(), 6, "夹具：本批之前的 6 列形状");

    let got = store::memory_read(&h(), &json!({ "key": "old.k" })).unwrap();
    assert_eq!(got["value"], json!("old-value"), "迁移不许丢行");
    assert_eq!(got["updated_at"], json!(123), "原时间戳不动");
    let c = cols(&conn);
    assert!(
        c.iter().any(|x| x == "hit_count") && c.iter().any(|x| x == "last_hit_at"),
        "{c:?}"
    );
    assert_eq!(learning(&conn, "old.k"), (0, None), "新列缺省 = 0 / NULL");

    store::memory_read(&h(), &json!({ "key": "old.k" })).unwrap();
    assert_eq!(
        learning(&conn, "old.k"),
        (0, None),
        "二次访问没有动静（幂等）"
    );
}

/// 半迁状态（有 `hit_count`、缺 `last_hit_at`）要**自愈**：逐列补法缺哪条补哪条；
/// "缺一就两条都跑"的写法会撞 `duplicate column name` ⇒ Err ⇒ 库永远打不开。
/// 这条是该设计的独有点（变异把逐列判断改成整批两条 ⇒ 只有它红）。
#[test]
fn half_migrated_library_self_heals_instead_of_locking() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("learn-half");
    let db = dir.join("memory.sqlite3");
    make_db(&db, "\n             hit_count INTEGER NOT NULL DEFAULT 0,");
    let conn = Connection::open(&db).unwrap();
    assert!(cols(&conn).iter().any(|x| x == "hit_count"));

    let got = store::memory_read(&h(), &json!({ "key": "old.k" })).unwrap();
    assert_eq!(got["value"], json!("old-value"), "半迁库照样能读，不许 Err");
    assert!(
        cols(&conn).iter().any(|x| x == "last_hit_at"),
        "缺的那条补上了"
    );
    assert_eq!(learning(&conn, "old.k"), (0, None));
}

/// 插件写路径（`memory_write` 的 `ON CONFLICT DO UPDATE SET` 显式列）不清学习量。
/// ML §6.4 的白名单口径：md 是人读投影、学习量是机器状态，二者不同源。
#[test]
fn learning_columns_survive_plugin_write() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("learn-survive");
    let db = dir.join("memory.sqlite3");
    std::fs::create_dir_all(&dir).unwrap();
    store::memory_write(&h(), &json!({ "key": "k.b", "value": "v1" })).unwrap();
    let conn = Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE memories SET hit_count = 5, last_hit_at = 888 WHERE key = 'k.b'",
        [],
    )
    .unwrap();

    store::memory_write(&h(), &json!({ "key": "k.b", "value": "v2" })).unwrap();
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "k.b" })).unwrap()["value"],
        json!("v2"),
        "值照旧覆盖"
    );
    assert_eq!(
        learning(&conn, "k.b"),
        (5, Some(888)),
        "同键覆盖不许清学习量"
    );
}
