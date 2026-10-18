//! **补偿入口**（台账 T35）：`memory_undo` 与它用的 `restore_entry`（整条还原，
//! 连旧 `updated_at` 与原 `workspace` 键位一起忠实还原——"恢复时间/当前位置"不是原状）。
//! 载荷两种形态都容错：标准契约 `{"output": {...}}` 与平铺的 output 字段。
//!
//! 与 `tools` 片同样经 `use super::mdsync;` 与 `super::schema::open`；`restore_entry` 只有
//! 本片调用，保持私有。这条通路是"不可逆动作默认要审批"那八条纪律里的补偿幂等面，
//! 搬动没碰它的判断顺序。

use plugin_api::serde_json::{Value, json};
use rusqlite::{Connection, params};

use super::mdsync;
use super::schema::open;
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
            let mut out = json!({ "key": key, "undone": true, "removed": removed > 0 });
            if removed > 0 {
                if let Some(w) = mdsync::sync_delete(&ws, key) {
                    out["md_warning"] = json!(w);
                }
            }
            return Ok(out);
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
        let mut out = json!({ "key": key, "undone": true, "restored": true });
        if let Some(w) = mdsync::sync_write(at_ws, key, value, tags, updated_at) {
            out["md_warning"] = json!(w);
        }
        return Ok(out);
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
        let mut out = json!({ "key": key, "undone": true, "restored": true });
        if let Some(w) = mdsync::sync_write(at_ws, key, value, tags, updated_at) {
            out["md_warning"] = json!(w);
        }
        return Ok(out);
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
