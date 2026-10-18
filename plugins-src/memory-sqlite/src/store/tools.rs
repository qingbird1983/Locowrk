//! **五枚记忆工具的入口**（台账 T35，S1 刀③b 起读写的就是真源 md）：
//! `memory_write` / `memory_read` / `memory_list` / `memory_search` / `memory_delete`
//! ——`lib.rs` 的分发表按名字派到这里，第六枚 `memory_undo`（补偿入口）在 `undo` 片。
//!
//! 这一片**一条 SQL 都不发**：`rusqlite` 与 `store/schema.rs`（建表 + 两份旧库迁移）
//! 随本刀退役，键位裁决仍走 `rows`→`facts` 那件（`write_scope` 的判断逻辑一字未动，
//! 只是它下面接的存储换了）。工具名字、参数面与返回字段名保持原形 ⇒ 提示词、审批面
//! 与既有会话历史都不会因为"换存储"而漂。
//!
//! 写侧的失败语义与宿主 S1 刀②c-1 同一条律：**先落真源，失败就是失败**（`Err` 一路
//! 上抛到工具结果）。旧形状"sqlite 已落、md 只是同步、失败只 warn"连同那个
//! `md_warning` 字段一起消失——没有第二份存储，就没有"另一半没同步上"这种状态。

use plugin_api::args::arg_u64;
use plugin_api::memlayout;
use plugin_api::serde_json::{Value, json};

use crate::arg_str;

use super::facts::{self, read_one};
use super::limits::{KEY_MAX, VALUE_MAX};
pub(crate) fn memory_write(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let value = arg_str(args, "value")?;
    let tags = args.get("tags").and_then(Value::as_str).unwrap_or("");
    // 四类型（S1 刀③a）里唯一由模型报的一维；值域外与没给都收进未分类。
    let ty = memlayout::normalize_type(args.get("type").and_then(Value::as_str).unwrap_or(""));
    let ws = super::facts::write_scope(ctx, args.get("scope"), &ty)?;
    // 三条校验的次序与错误文案照退役掉的 SQL 版逐字保留（上限是"能被写进来的东西
    // 有多大"这一件事，与存储换没换无关）。
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
    let r = facts::root()?;
    // 覆盖前的快照 = undo 载荷。它从真源读，所以天生带 `type`（旧形状里这个载荷
    // 取自库行，那一维根本不存在）。
    let previous = read_one(&r, &ws, &key)?.map(|h| h.json());
    let at = facts::now();
    memlayout::write_fact(
        &r,
        &memlayout::FactEntry {
            workspace: &ws,
            key: &key,
            value: &value,
            tags,
            source: "explicit",
            updated_at: at,
            ty: &ty,
        },
    )
    .map_err(|e| format!("memory_write `{key}`: {e}"))?;
    let scope = json!(if ws.is_empty() { "global" } else { "project" });
    let mut out = json!({
        "key": key,
        "scope": scope,
        "type": ty,
        "created": previous.is_none(),
        "previous": previous,
    });
    // `Why:` / `How to apply:` 的纪律是提醒不是拒绝：先落盘，再把缺的那一段报回去。
    if let Some(h) = memlayout::missing_guidance(&ty, &value) {
        out["hint"] = json!(h);
    }
    Ok(out)
}

pub(crate) fn memory_read(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let r = facts::root()?;
    // 归属会话：项目键位优先，未命中回落全局偏好层（口径见 `facts::read_preferred`）
    match facts::read_preferred(&r, ctx.scope(), &key)? {
        Some(h) => {
            let mut entry = h.json();
            if let Value::Object(map) = &mut entry {
                map.insert("found".into(), json!(true));
            }
            Ok(entry)
        }
        None => Ok(json!({ "found": false, "key": key })),
    }
}

pub(crate) fn memory_list(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let prefix = args.get("prefix").and_then(Value::as_str).unwrap_or("");
    let limit = arg_u64(args, "limit", 50, 200)?;
    let offset = arg_u64(args, "offset", 0, u64::MAX)?;
    let r = facts::root()?;
    // 双层：本项目键位 ∪ 全局偏好层（未归属/headless 两条件重合 = 只 ''）。
    // 同名 key 两层各有一条时**两条都算**，与退役掉的那条 `OR workspace = ''` 同形。
    let mut hits = facts::both_layers(&r, ctx.scope())?;
    hits.retain(|h| h.fact.key.starts_with(prefix));
    // `ORDER BY key`：SQLite TEXT 的默认 BINARY 排序 = 字节序，Rust 的 `String` 序也是
    // 字节序 ⇒ 这一条**大小写敏感**，与 `memory_search` 那条不敏感的不是同一口径
    // （那一条在 `memlayout::fact_matches` 里折叠，理由写在那片）。
    hits.sort_by(|a, b| a.fact.key.cmp(&b.fact.key));
    let total = hits.len();
    let entries: Vec<Value> = hits
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .map(|h| h.json())
        .collect();
    Ok(json!({ "total": total, "entries": entries }))
}

pub(crate) fn memory_search(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let query = arg_str(args, "query")?;
    let limit = arg_u64(args, "limit", 20, 100)?;
    let r = facts::root()?;
    let mut hits = facts::both_layers(&r, ctx.scope())?;
    // 三维子串 + ASCII 大小写不敏感 + 通配符按字面量：这一条谓词与宿主管理页的列举
    // 过滤共用同一个函数（`memlayout::fact_matches`），两处各写一份迟早漂成
    // "管理页搜得到、工具搜不到"。
    hits.retain(|h| memlayout::fact_matches(&h.fact, &query));
    hits.sort_by(|a, b| {
        b.fact
            .updated_at
            .cmp(&a.fact.updated_at)
            .then_with(|| a.fact.key.cmp(&b.fact.key))
    });
    let matches: Vec<Value> = hits
        .into_iter()
        .take(limit as usize)
        .map(|h| h.json())
        .collect();
    Ok(json!({ "query": query, "matches": matches }))
}

pub(crate) fn memory_delete(ctx: &super::WsCtx, args: &Value) -> Result<Value, String> {
    let key = arg_str(args, "key")?;
    let r = facts::root()?;
    // 双层纪律（Context 文档 §4.6）：'' 键位 = 全局偏好层，聊天工具删除只
    // 对 headless 开放（其单一宇宙本就是 ''）；桌面会话（归属/未归属）
    // 删全局行一律拒绝——全局偏好的增删是人的操作，走记忆管理页。
    if let super::WsCtx::Headless = ctx {
        let previous = read_one(&r, "", &key)?.map(|h| h.json());
        let removed = memlayout::delete_fact(&r, "", &key)
            .map_err(|e| format!("memory_delete `{key}`: {e}"))?;
        return Ok(json!({ "key": key, "deleted": removed, "previous_entry": previous }));
    }
    let ws = ctx.scope();
    if !ws.is_empty() {
        if let Some(h) = read_one(&r, ws, &key)? {
            let previous = h.json();
            let removed = memlayout::delete_fact(&r, ws, &key)
                .map_err(|e| format!("memory_delete `{key}`: {e}"))?;
            return Ok(json!({ "key": key, "deleted": removed, "previous_entry": previous }));
        }
    }
    if read_one(&r, "", &key)?.is_some() {
        return Err(format!(
            "memory_delete `{key}`: exists only as a global user preference — global \
             entries are managed from the human memory management page, not deletable \
             from chat"
        ));
    }
    Ok(json!({ "key": key, "deleted": false, "previous_entry": null }))
}
