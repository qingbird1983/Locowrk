//! SQLite 存储实现。连接按调用打开、用完即弃（host 侧 spawn_blocking 串
//! 调用，单文件库足够；busy_timeout 兜底并发），表结构惰性创建。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use plugin_api::serde_json::{Value, json};
use rusqlite::{Connection, Error, params};

use crate::{arg_str, arg_u64};

/// key / value 上限（字节）。key 是主键不能无限长；value 防 runaway 写。
const KEY_MAX: usize = 512;
const VALUE_MAX: usize = 256 * 1024;
/// LIKE 通配符转义符（SQL 语法层面）。
const ESC: char = '\\';

fn open() -> Result<Connection, String> {
    let dir = plugin_api::fsutil::data_root()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("memory db dir `{}`: {e}", dir.display()))?;
    let conn =
        Connection::open(dir.join("memory.sqlite3")).map_err(|e| format!("memory db open: {e}"))?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|e| format!("memory db busy_timeout: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memories (
             key        TEXT PRIMARY KEY,
             value      TEXT NOT NULL,
             tags       TEXT NOT NULL DEFAULT '',
             updated_at INTEGER NOT NULL
         );",
    )
    .map_err(|e| format!("memory db init: {e}"))?;
    Ok(conn)
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// LIKE 通配符按字面量匹配（% 和 _ 是用户数据，不是模式）。
fn escape_like(s: &str) -> String {
    s.replace(ESC, "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn row_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "key": r.get::<_, String>(0)?,
        "value": r.get::<_, String>(1)?,
        "tags": r.get::<_, String>(2)?,
        "updated_at": r.get::<_, i64>(3)?,
    }))
}

pub(crate) fn memory_put(args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let value = arg_str(args, "value")?;
    let tags = args.get("tags").and_then(Value::as_str).unwrap_or("");
    if key.is_empty() {
        return Err("memory_put: `key` must not be empty".into());
    }
    if key.len() > KEY_MAX {
        return Err(format!(
            "memory_put: `key` is {} bytes, over the {KEY_MAX} limit",
            key.len()
        ));
    }
    if value.len() > VALUE_MAX {
        return Err(format!(
            "memory_put: `value` is {} bytes, over the {VALUE_MAX} limit",
            value.len()
        ));
    }
    let conn = open()?;
    let previous = conn
        .query_row(
            "SELECT value, tags, updated_at FROM memories WHERE key = ?1",
            params![key],
            |r| Ok(json!({ "value": r.get::<_, String>(0)?, "tags": r.get::<_, String>(1)?, "updated_at": r.get::<_, i64>(2)? })),
        )
        .ok();
    conn.execute(
        "INSERT INTO memories (key, value, tags, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, tags = excluded.tags,
                                        updated_at = excluded.updated_at",
        params![key, value, tags, now()],
    )
    .map_err(|e| format!("memory_put `{key}`: {e}"))?;
    Ok(json!({ "key": key, "created": previous.is_none(), "previous": previous }))
}

pub(crate) fn memory_get(args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let conn = open()?;
    match conn.query_row(
        "SELECT key, value, tags, updated_at FROM memories WHERE key = ?1",
        params![key],
        row_json,
    ) {
        Ok(mut entry) => {
            if let Value::Object(map) = &mut entry {
                map.insert("found".into(), json!(true));
            }
            Ok(entry)
        }
        Err(Error::QueryReturnedNoRows) => Ok(json!({ "found": false, "key": key })),
        Err(e) => Err(format!("memory_get `{key}`: {e}")),
    }
}

pub(crate) fn memory_list(args: &Value) -> Result<Value, String> {
    let prefix = args.get("prefix").and_then(Value::as_str).unwrap_or("");
    let limit = arg_u64(args, "limit", 50, 200)?;
    let offset = arg_u64(args, "offset", 0, u64::MAX)?;
    let conn = open()?;
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE (?1 = '' OR key LIKE ?1 || '%' ESCAPE '\\')",
            params![prefix],
            |r| r.get(0),
        )
        .map_err(|e| format!("memory_list: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT key, value, tags, updated_at FROM memories
             WHERE (?1 = '' OR key LIKE ?1 || '%' ESCAPE '\\')
             ORDER BY key LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| format!("memory_list: {e}"))?;
    let entries: Vec<Value> = stmt
        .query_map(params![prefix, limit, offset], row_json)
        .map_err(|e| format!("memory_list: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("memory_list: {e}"))?;
    Ok(json!({ "total": total, "entries": entries }))
}

pub(crate) fn memory_search(args: &Value) -> Result<Value, String> {
    let query = arg_str(args, "query")?;
    let limit = arg_u64(args, "limit", 20, 100)?;
    let pattern = format!("%{}%", escape_like(&query));
    let conn = open()?;
    let mut stmt = conn
        .prepare(
            "SELECT key, value, tags, updated_at FROM memories
             WHERE key LIKE ?1 ESCAPE '\\' OR value LIKE ?1 ESCAPE '\\'
                OR tags LIKE ?1 ESCAPE '\\'
             ORDER BY updated_at DESC, key LIMIT ?2",
        )
        .map_err(|e| format!("memory_search: {e}"))?;
    let matches: Vec<Value> = stmt
        .query_map(params![pattern, limit], row_json)
        .map_err(|e| format!("memory_search: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("memory_search: {e}"))?;
    Ok(json!({ "query": query, "matches": matches }))
}

pub(crate) fn memory_delete(args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let conn = open()?;
    let previous = conn
        .query_row(
            "SELECT key, value, tags, updated_at FROM memories WHERE key = ?1",
            params![key],
            row_json,
        )
        .ok();
    let deleted = conn
        .execute("DELETE FROM memories WHERE key = ?1", params![key])
        .map_err(|e| format!("memory_delete `{key}`: {e}"))?;
    Ok(json!({ "key": key, "deleted": deleted > 0, "previous_entry": previous }))
}

/// undo 入口。容错两种形态：标准契约 `{"output": {...}}`（内核补偿层传的），
/// 或把 output 字段平铺在 args 顶层（便于手工/直调排查）。
pub(crate) fn memory_undo(args: &Value) -> Result<Value, String> {
    let output = args.get("output").unwrap_or(args);
    let conn = open()?;
    if let Some(prev) = output.get("previous") {
        // put 的撤销：previous 为 null → 删掉当时新建的 key；否则还原旧条目
        let key = output.get("key").and_then(Value::as_str).ok_or_else(|| {
            "memory_undo: missing `output.key` (pass the original memory_put result)".to_string()
        })?;
        if prev.is_null() {
            let removed = conn
                .execute("DELETE FROM memories WHERE key = ?1", params![key])
                .map_err(|e| format!("memory_undo `{key}`: {e}"))?;
            return Ok(json!({ "key": key, "undone": true, "removed": removed > 0 }));
        }
        let value = prev
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| "memory_undo: previous entry missing `value`".to_string())?;
        let tags = prev.get("tags").and_then(Value::as_str).unwrap_or("");
        let updated_at = prev
            .get("updated_at")
            .and_then(Value::as_i64)
            .ok_or_else(|| "memory_undo: previous entry missing `updated_at`".to_string())?;
        restore_entry(&conn, key, value, tags, updated_at)?;
        return Ok(json!({ "key": key, "undone": true, "restored": true }));
    }
    if let Some(entry) = output.get("previous_entry") {
        // delete 的撤销：条目缺失（本来就不存在）→ no-op；否则整条还原
        if entry.is_null() {
            return Ok(json!({ "undone": true, "noop": true }));
        }
        let key = entry
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| "memory_undo: previous_entry missing `key`".to_string())?;
        let value = entry
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| "memory_undo: previous_entry missing `value`".to_string())?;
        let tags = entry.get("tags").and_then(Value::as_str).unwrap_or("");
        let updated_at = entry
            .get("updated_at")
            .and_then(Value::as_i64)
            .ok_or_else(|| "memory_undo: previous_entry missing `updated_at`".to_string())?;
        restore_entry(&conn, key, value, tags, updated_at)?;
        return Ok(json!({ "key": key, "undone": true, "restored": true }));
    }
    Err(
        "memory_undo: missing `output.previous` or `output.previous_entry` (pass the original \
         memory_put / memory_delete result)"
            .to_string(),
    )
}

/// 整条还原（含旧 updated_at——忠实还原，而不是把"恢复时间"当成更新时间）。
fn restore_entry(
    conn: &Connection,
    key: &str,
    value: &str,
    tags: &str,
    updated_at: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO memories (key, value, tags, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, tags = excluded.tags,
                                        updated_at = excluded.updated_at",
        params![key, value, tags, updated_at],
    )
    .map_err(|e| format!("memory_undo `{key}`: {e}"))?;
    Ok(())
}
