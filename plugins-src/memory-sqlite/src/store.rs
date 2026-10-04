//! SQLite 存储实现。连接按调用打开、用完即弃（host 侧 spawn_blocking 串
//! 调用，单文件库足够；busy_timeout 兜底并发），表结构惰性创建。
//!
//! **项目隔离（㉘）**：主键 = `(workspace, key)`——`workspace` 是调用会话
//! 归属的项目根（经宿主 vtable `session_context` 查询；`""` = 未归属）。
//! 同名 key 在不同项目下互不覆盖、互不可见（Context 文档 §4.6 隔离规则）。
//! 旧库（key 单主键、无 workspace 列）首次访问时迁移：整表重建，旧行归入
//! 未归属键位。宿主侧（desktop memory.rs）同持一份等价迁移——两侧建表
//! 形状一致（含宿主演进的 `source` 列），先跑谁结果都一样。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use plugin_api::serde_json::{Value, json};
use rusqlite::{Connection, Error, params};

use crate::{arg_str, arg_u64};

/// key / value 上限（字节）。key 是主键不能无限长；value 防 runaway 写。
const KEY_MAX: usize = 512;
const VALUE_MAX: usize = 256 * 1024;
/// LIKE 通配符转义符（SQL 语法层面）。
const ESC: char = '\\';

/// 当前权威表形状（两侧同源；`source` 列归宿主演进，缺省 explicit）。
const CREATE_TABLE: &str = "CREATE TABLE memories (
    workspace  TEXT NOT NULL DEFAULT '',
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    tags       TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL,
    source     TEXT NOT NULL DEFAULT 'explicit',
    PRIMARY KEY (workspace, key)
);";

fn open() -> Result<Connection, String> {
    let dir = plugin_api::fsutil::data_root()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("memory db dir `{}`: {e}", dir.display()))?;
    let conn =
        Connection::open(dir.join("memory.sqlite3")).map_err(|e| format!("memory db open: {e}"))?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|e| format!("memory db busy_timeout: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memories (
             workspace  TEXT NOT NULL DEFAULT '',
             key        TEXT NOT NULL,
             value      TEXT NOT NULL,
             tags       TEXT NOT NULL DEFAULT '',
             updated_at INTEGER NOT NULL,
             source     TEXT NOT NULL DEFAULT 'explicit',
             PRIMARY KEY (workspace, key)
         );",
    )
    .map_err(|e| format!("memory db init: {e}"))?;
    ensure_workspace_schema(&conn)?;
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

/// 带 `workspace` 列的行（双层记忆：读/搜/列的结果带层标 scope）。
fn row_scoped_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let ws: String = r.get(4)?;
    let mut v = row_json(r)?;
    if let Value::Object(map) = &mut v {
        map.insert(
            "scope".into(),
            json!(if ws.is_empty() { "global" } else { "project" }),
        );
    }
    Ok(v)
}

/// 键位内存在性探测（delete 的全局/项目归属裁决用）。
fn exists(conn: &Connection, ws: &str, key: &str) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE workspace = ?1 AND key = ?2",
            params![ws, key],
            |r| r.get(0),
        )
        .map_err(|e| format!("memory_delete `{key}`: {e}"))?;
    Ok(n > 0)
}

/// 写入键位裁决（Context 文档 §4.6 双层记忆）：缺省 = 当前项目；显式
/// `"global"` = 全局偏好层（`''`）；未归属会话缺省写**拒绝**（没有项目
/// 的"项目记忆"不许落地污染全局层——先归属，或显式升格为偏好）；
/// headless 无会话上下文 = `''` 单一宇宙（旧行为）。
fn write_scope(ctx: &super::WsCtx, scope_arg: Option<&Value>) -> Result<String, String> {
    let want_global = match scope_arg {
        None | Some(Value::Null) => false,
        Some(v) => match v.as_str() {
            Some("project") => false,
            Some("global") => true,
            other => {
                return Err(format!(
                    "memory_write: `scope` must be \"project\" or \"global\", got {other:?}"
                ));
            }
        },
    };
    Ok(match ctx {
        super::WsCtx::Headless => String::new(),
        super::WsCtx::Project(_) if want_global => String::new(),
        super::WsCtx::Project(ws) => ws.clone(),
        super::WsCtx::Unattributed if want_global => String::new(),
        super::WsCtx::Unattributed => {
            return Err(
                "memory_write: this session has no project — ask the user to associate one \
                 first, or pass scope=\"global\" to save a user preference that applies in \
                 every project"
                    .into(),
            );
        }
    })
}

/// undo 契约载荷里的整行快照（含 workspace——undo 忠实还原原键位）。
fn row_with_workspace_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "workspace": r.get::<_, String>(0)?,
        "key": r.get::<_, String>(1)?,
        "value": r.get::<_, String>(2)?,
        "tags": r.get::<_, String>(3)?,
        "updated_at": r.get::<_, i64>(4)?,
    }))
}

pub(crate) fn memory_write(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let value = arg_str(args, "value")?;
    let tags = args.get("tags").and_then(Value::as_str).unwrap_or("");
    let ws = write_scope(ctx, args.get("scope"))?;
    if key.is_empty() {
        return Err("memory_write: `key` must not be empty".into());
    }
    if key.len() > KEY_MAX {
        return Err(format!(
            "memory_write: `key` is {} bytes, over the {KEY_MAX} limit",
            key.len()
        ));
    }
    if value.len() > VALUE_MAX {
        return Err(format!(
            "memory_write: `value` is {} bytes, over the {VALUE_MAX} limit",
            value.len()
        ));
    }
    let conn = open()?;
    let previous = conn
        .query_row(
            "SELECT workspace, key, value, tags, updated_at FROM memories \
             WHERE workspace = ?1 AND key = ?2",
            params![ws, key],
            row_with_workspace_json,
        )
        .ok();
    conn.execute(
        "INSERT INTO memories (workspace, key, value, tags, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(workspace, key) DO UPDATE SET value = excluded.value,
             tags = excluded.tags, updated_at = excluded.updated_at",
        params![ws, key, value, tags, now()],
    )
    .map_err(|e| format!("memory_write `{key}`: {e}"))?;
    let scope = json!(if ws.is_empty() { "global" } else { "project" });
    Ok(json!({ "key": key, "scope": scope, "created": previous.is_none(), "previous": previous }))
}

pub(crate) fn memory_read(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let conn = open()?;
    // 归属会话：项目键位优先，未命中回落全局偏好层；未归属/headless 的
    // 当前键位本来就是 ''（IN ('', '') 恒等）
    match conn.query_row(
        "SELECT key, value, tags, updated_at, workspace FROM memories \
         WHERE key = ?2 AND workspace IN (?1, '') \
         ORDER BY (workspace = ?1) DESC LIMIT 1",
        params![ctx.scope(), key],
        row_scoped_json,
    ) {
        Ok(mut entry) => {
            if let Value::Object(map) = &mut entry {
                map.insert("found".into(), json!(true));
            }
            Ok(entry)
        }
        Err(Error::QueryReturnedNoRows) => Ok(json!({ "found": false, "key": key })),
        Err(e) => Err(format!("memory_read `{key}`: {e}")),
    }
}

pub(crate) fn memory_list(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let prefix = args.get("prefix").and_then(Value::as_str).unwrap_or("");
    let limit = arg_u64(args, "limit", 50, 200)?;
    let offset = arg_u64(args, "offset", 0, u64::MAX)?;
    let conn = open()?;
    // 双层：本项目键位 ∪ 全局偏好层（未归属/headless 两条件重合 = 只 ''）
    let filter = "WHERE (workspace = ?1 OR workspace = '') \
                  AND (?2 = '' OR key LIKE ?2 || '%' ESCAPE '\\')";
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM memories {filter}"),
            params![ctx.scope(), prefix],
            |r| r.get(0),
        )
        .map_err(|e| format!("memory_list: {e}"))?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT key, value, tags, updated_at, workspace FROM memories {filter}
             ORDER BY key LIMIT ?3 OFFSET ?4"
        ))
        .map_err(|e| format!("memory_list: {e}"))?;
    let entries: Vec<Value> = stmt
        .query_map(params![ctx.scope(), prefix, limit, offset], row_scoped_json)
        .map_err(|e| format!("memory_list: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("memory_list: {e}"))?;
    Ok(json!({ "total": total, "entries": entries }))
}

pub(crate) fn memory_search(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let query = arg_str(args, "query")?;
    let limit = arg_u64(args, "limit", 20, 100)?;
    let pattern = format!("%{}%", escape_like(&query));
    let conn = open()?;
    let mut stmt = conn
        .prepare(
            "SELECT key, value, tags, updated_at, workspace FROM memories
             WHERE (workspace = ?1 OR workspace = '')
                AND (key LIKE ?2 ESCAPE '\\' OR value LIKE ?2 ESCAPE '\\'
                     OR tags LIKE ?2 ESCAPE '\\')
             ORDER BY updated_at DESC, key LIMIT ?3",
        )
        .map_err(|e| format!("memory_search: {e}"))?;
    let matches: Vec<Value> = stmt
        .query_map(params![ctx.scope(), pattern, limit], row_scoped_json)
        .map_err(|e| format!("memory_search: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("memory_search: {e}"))?;
    Ok(json!({ "query": query, "matches": matches }))
}

pub(crate) fn memory_delete(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let conn = open()?;
    // 双层纪律（Context 文档 §4.6）：'' 键位 = 全局偏好层，聊天工具删除只
    // 对 headless 开放（其单一宇宙本就是 ''）；桌面会话（归属/未归属）
    // 删全局行一律拒绝——全局偏好的增删是人的操作，走记忆管理页。
    if let super::WsCtx::Headless = ctx {
        let previous = conn
            .query_row(
                "SELECT workspace, key, value, tags, updated_at FROM memories \
                 WHERE workspace = '' AND key = ?1",
                params![key],
                row_with_workspace_json,
            )
            .ok();
        let deleted = conn
            .execute(
                "DELETE FROM memories WHERE workspace = '' AND key = ?1",
                params![key],
            )
            .map_err(|e| format!("memory_delete `{key}`: {e}"))?;
        return Ok(json!({ "key": key, "deleted": deleted > 0, "previous_entry": previous }));
    }
    let ws = ctx.scope();
    let in_project = !ws.is_empty() && exists(&conn, ws, &key)?;
    if in_project {
        let previous = conn
            .query_row(
                "SELECT workspace, key, value, tags, updated_at FROM memories \
                 WHERE workspace = ?1 AND key = ?2",
                params![ws, key],
                row_with_workspace_json,
            )
            .ok();
        conn.execute(
            "DELETE FROM memories WHERE workspace = ?1 AND key = ?2",
            params![ws, key],
        )
        .map_err(|e| format!("memory_delete `{key}`: {e}"))?;
        return Ok(json!({ "key": key, "deleted": true, "previous_entry": previous }));
    }
    if exists(&conn, "", &key)? {
        return Err(format!(
            "memory_delete `{key}`: exists only as a global user preference — global \
             entries are managed from the human memory management page, not deletable \
             from chat"
        ));
    }
    Ok(json!({ "key": key, "deleted": false, "previous_entry": null }))
}

/// undo 入口。容错两种形态：标准契约 `{"output": {...}}`（内核补偿层传的），
/// 或把 output 字段平铺在 args 顶层（便于手工/直调排查）。快照载荷带
/// `workspace` 时忠实还原原键位；旧载荷（无 workspace）回落当前会话键位
/// （未归属/headless = `''`）。补偿是审批后的机械还原，不受全局禁删约束。
pub(crate) fn memory_undo(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let ws = ctx.scope().to_string();
    let output = args.get("output").unwrap_or(args);
    let conn = open()?;
    if let Some(prev) = output.get("previous") {
        // write 的撤销：previous 为 null → 删掉当时新建的 key；否则还原旧条目
        let key = output.get("key").and_then(Value::as_str).ok_or_else(|| {
            "memory_undo: missing `output.key` (pass the original memory_write result)".to_string()
        })?;
        if prev.is_null() {
            let removed = conn
                .execute(
                    "DELETE FROM memories WHERE workspace = ?1 AND key = ?2",
                    params![ws, key],
                )
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
        let at_ws = prev
            .get("workspace")
            .and_then(Value::as_str)
            .unwrap_or(ws.as_str());
        restore_entry(&conn, at_ws, key, value, tags, updated_at)?;
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
        let at_ws = entry
            .get("workspace")
            .and_then(Value::as_str)
            .unwrap_or(ws.as_str());
        restore_entry(&conn, at_ws, key, value, tags, updated_at)?;
        return Ok(json!({ "key": key, "undone": true, "restored": true }));
    }
    Err(
        "memory_undo: missing `output.previous` or `output.previous_entry` (pass the original \
         memory_write / memory_delete result)"
            .to_string(),
    )
}

/// 整条还原（含旧 updated_at 与原 workspace 键位——忠实还原，而不是把
/// "恢复时间/当前位置"当成原状）。
fn restore_entry(
    conn: &Connection,
    ws: &str,
    key: &str,
    value: &str,
    tags: &str,
    updated_at: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO memories (workspace, key, value, tags, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(workspace, key) DO UPDATE SET value = excluded.value,
             tags = excluded.tags, updated_at = excluded.updated_at",
        params![ws, key, value, tags, updated_at],
    )
    .map_err(|e| format!("memory_undo `{key}`: {e}"))?;
    Ok(())
}
