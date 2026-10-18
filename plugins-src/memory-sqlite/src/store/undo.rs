//! **补偿入口**（台账 T35；S1 刀③b 起它还原的就是真源 md）：`memory_undo` 与它用的
//! `restore_entry`——整条还原，连旧 `updated_at`、原 `workspace` 键位与 `type` 分类一起
//! 忠实还原（"恢复时间 / 当前位置"不是原状）。载荷两种形态都容错：标准契约
//! `{"output": {...}}` 与平铺的 output 字段。
//!
//! 这条通路是"不可逆动作默认要审批"那八条纪律里的补偿幂等面。搬动没碰它的判断顺序，
//! 两个分支的**报错文案**也逐字留着（补偿层按这些串排查过问题）。

use std::path::Path;

use plugin_api::memlayout;
use plugin_api::serde_json::{Value, json};

use super::facts;

/// 载荷里分类那一维的取法。刀③a 之前写下的**旧载荷没有 `type` 字段**（那时这个快照
/// 取自库行，而库里没有这一列）⇒ 缺省读成未分类。跨版本恢复一次撤销的后果如实登记在
/// 这里：那条记忆的**内容**忠实回来，类型退回未分类——人打开文件看得见、再改一次就
/// 回来。不拿现存文件里的分类顶替：撤销要还原的是载荷那一刻的原状。
fn payload_type(entry: &Value) -> String {
    entry
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// undo 入口。容错两种形态：标准契约 `{"output": {...}}`（内核补偿层传的），
/// 或把 output 字段平铺在 args 顶层（便于手工/直调排查）。快照载荷带
/// `workspace` 时忠实还原原键位；旧载荷（无 workspace）回落当前会话键位
/// （未归属/headless = `''`）。补偿是审批后的机械还原，不受全局禁删约束。
pub(crate) fn memory_undo(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let ws = ctx.scope().to_string();
    let output = args.get("output").unwrap_or(args);
    let r = facts::root()?;
    if let Some(prev) = output.get("previous") {
        // write 的撤销：previous 为 null → 删掉当时新建的 key；否则还原旧条目
        let key = output.get("key").and_then(Value::as_str).ok_or_else(|| {
            "memory_undo: missing `output.key` (pass the original memory_write result)".to_string()
        })?;
        if prev.is_null() {
            // 撤销一次**新建** = 当作没发生过 ⇒ 这里是全仓唯一容许的硬删调用点
            // （闸 `SS15a` 的白名单就它一个）。为什么不进 `obsolete/`：回收区的用途是
            // "用户删错了想找回"，而这一条他从来没见过（写入刚被回滚），把它留在回收区
            // 会让那个目录变成补偿日志。撤销"覆盖写"（下面那一支）与撤销"删除"
            // （`previous_entry`）都是往真源写回，不涉及删除形状。
            let removed = memlayout::delete_fact(&r, &ws, key)
                .map_err(|e| format!("memory_undo `{key}`: {e}"))?;
            return Ok(json!({ "key": key, "undone": true, "removed": removed }));
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
        let ty = payload_type(prev);
        restore_entry(&r, at_ws, key, value, tags, &ty, updated_at)?;
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
        let ty = payload_type(entry);
        restore_entry(&r, at_ws, key, value, tags, &ty, updated_at)?;
        return Ok(json!({ "key": key, "undone": true, "restored": true }));
    }
    Err(
        "memory_undo: missing `output.previous` or `output.previous_entry` (pass the original \
         memory_write / memory_delete result)"
            .to_string(),
    )
}

/// 整条还原（含旧 updated_at、原 workspace 键位与分类）。真源写失败 = `Err`：
/// 撤销撤到一半必须说出来，不许报"已还原"。
fn restore_entry(
    r: &Path,
    ws: &str,
    key: &str,
    value: &str,
    tags: &str,
    ty: &str,
    updated_at: i64,
) -> Result<(), String> {
    memlayout::write_fact(
        r,
        &memlayout::FactEntry {
            workspace: ws,
            key,
            value,
            tags,
            source: "explicit",
            updated_at,
            ty,
        },
    )
    .map(|_| ())
    .map_err(|e| format!("memory_undo `{key}`: {e}"))
}
