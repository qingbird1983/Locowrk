//! 工具入口共用的**读侧件与键位裁决**（台账 T35）：时间戳 `now`、LIKE 转义
//! `escape_like`、三种行→JSON 投影（`row_json` / `row_scoped_json` 带层标 /
//! `row_with_workspace_json` 给 undo 载荷）、存在性探测 `exists` 与写入键位裁决
//! `write_scope`（双层记忆纪律的落点：未归属会话缺省写**拒绝**）。
//!
//! 这一片是"五个工具入口之下共用的几件"，不是六个各自独立的小文件：每件都只有几行，
//! 再拆就是为行数造结构（T31 那把 `parse_usage` 的课）。被工具片读的七个入口抬
//! `pub(super)`，只有本片内互用的 `row_json` 保持私有。

use std::time::{SystemTime, UNIX_EPOCH};

use plugin_api::serde_json::{Value, json};
use rusqlite::{Connection, params};

use super::limits::ESC;
pub(super) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// LIKE 通配符按字面量匹配（% 和 _ 是用户数据，不是模式）。
pub(super) fn escape_like(s: &str) -> String {
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
pub(super) fn row_scoped_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
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
pub(super) fn exists(conn: &Connection, ws: &str, key: &str) -> Result<bool, String> {
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
pub(super) fn write_scope(ctx: &super::WsCtx, scope_arg: Option<&Value>) -> Result<String, String> {
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
pub(super) fn row_with_workspace_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "workspace": r.get::<_, String>(0)?,
        "key": r.get::<_, String>(1)?,
        "value": r.get::<_, String>(2)?,
        "tags": r.get::<_, String>(3)?,
        "updated_at": r.get::<_, i64>(4)?,
    }))
}
