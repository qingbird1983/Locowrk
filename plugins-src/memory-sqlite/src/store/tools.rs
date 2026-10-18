//! **五个记忆工具的入口**（台账 T35）：`memory_write` / `memory_read` / `memory_list` /
//! `memory_search` / `memory_delete`——`lib.rs` 的分发表按名字派到这里，SQL 与键位裁决
//! 全在 `store` 子树内，命令面不写业务。
//!
//! 读侧件来自 `rows`、上限来自 `limits`、开连接来自 `schema`、人类层双写经
//! `use super::mdsync;`（那个 mod 留在父文件里，见 `store.rs` 的说明）。键位上下文
//! `super::WsCtx` 写法和被搬之前逐字相同：装配文件里 `pub(crate) use crate::WsCtx;`
//! 把它接住 ⇒ 本刀**没有一处**是为了改路径而改正文。

use plugin_api::args::arg_u64;
use plugin_api::serde_json::{Value, json};
use rusqlite::{Error, params};

use crate::arg_str;

use super::limits::{KEY_MAX, VALUE_MAX};
use super::mdsync;
use super::rows::{
    escape_like, exists, now, row_scoped_json, row_with_workspace_json, write_scope,
};
use super::schema::open;
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
    let at = now();
    conn.execute(
        "INSERT INTO memories (workspace, key, value, tags, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(workspace, key) DO UPDATE SET value = excluded.value,
             tags = excluded.tags, updated_at = excluded.updated_at",
        params![ws, key, value, tags, at],
    )
    .map_err(|e| format!("memory_write `{key}`: {e}"))?;
    let scope = json!(if ws.is_empty() { "global" } else { "project" });
    let mut out =
        json!({ "key": key, "scope": scope, "created": previous.is_none(), "previous": previous });
    if let Some(w) = mdsync::sync_write(&ws, &key, &value, tags, at) {
        out["md_warning"] = json!(w);
    }
    Ok(out)
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
        let mut out = json!({ "key": key, "deleted": deleted > 0, "previous_entry": previous });
        if deleted > 0 {
            if let Some(w) = mdsync::sync_delete("", &key) {
                out["md_warning"] = json!(w);
            }
        }
        return Ok(out);
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
        let mut out = json!({ "key": key, "deleted": true, "previous_entry": previous });
        if let Some(w) = mdsync::sync_delete(ws, &key) {
            out["md_warning"] = json!(w);
        }
        return Ok(out);
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
