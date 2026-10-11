//! memory 三态用例：正常（roundtrip / undo 还原 / 搜索 / 列举）/ 边界
//! （缺失键 no-op、参数上限、缺字段拒绝）/ 恶意（SQL 注入惰性、LIKE
//! 通配符按字面量）/ 项目隔离（㉘：同名 key 各项目互不覆盖互不可见、
//! undo 忠实还原原键位、旧库迁移归入未归属）。涉 env（LOCOWRK_DATA_DIR）
//! 用例全程持锁串行。
//! 批 F-15 目录化：共享假件与 env 锁留本文件，用例按面分簇见 `kv_basic` /
//! `project_isolation` / `scope_two_layer` / `human_md_layer`。

mod human_md_layer;
mod kv_basic;
mod project_isolation;
mod scope_two_layer;

use super::WsCtx;
use super::store;
use plugin_api::serde_json::{Value, json};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// headless 上下文（'' 单一宇宙，旧行为）。
fn h() -> WsCtx {
    WsCtx::Headless
}

/// 归属项目上下文。
fn proj(ws: &str) -> WsCtx {
    WsCtx::Project(ws.into())
}

fn temp_data_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("locowrk-mem-{tag}-{nanos}-{}", std::process::id()));
    // SAFETY: 全部用例经 ENV_LOCK 串行；Rust 2024 起 set_var 为 unsafe
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &dir) };
    dir
}

/// 模拟内核补偿层的 undo 契约调用。
fn contract(call_id: &str, args: Value, output: Value) -> Value {
    json!({ "call_id": call_id, "args": args, "output": output })
}
