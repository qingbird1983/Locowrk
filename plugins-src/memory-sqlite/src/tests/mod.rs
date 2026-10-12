//! memory 三态用例：正常（roundtrip / undo 还原 / 搜索 / 列举）/ 边界
//! （缺失键 no-op、参数上限、缺字段拒绝）/ 恶意（SQL 注入惰性、LIKE
//! 通配符按字面量）/ 项目隔离（㉘：同名 key 各项目互不覆盖互不可见、
//! undo 忠实还原原键位、旧库迁移归入未归属）。涉 env（LOCOWRK_DATA_DIR）
//! 用例全程持锁串行。
//! 批 F-15 目录化：共享假件与 env 锁留本文件，用例按面分簇见 `kv_basic` /
//! `project_isolation` / `scope_two_layer` / `human_md_layer`；Q2（MR §6）加
//! `keyside_norm`（键位归一：同项目两种写法在库里只有一行）。

mod human_md_layer;
mod keyside_norm;
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

/// 归属项目上下文——**走生产的分诊入口**（`from_parts` 里含 Q2 的键位归一）。
///
/// 测试不许再手搭 `WsCtx::Project("…")`：那样归一化就只被 `wskey` 自己的单元
/// 判据看着，插件这一侧「写进去的键位到底是什么串」没有判据。走 `from_parts`
/// 之后，`proj("F:/ws/a")` 与 `proj("f:\\WS\\A\\")` 给的是同一个 ctx，这件事
/// 本身就有用例钉（见 `keyside_norm`）。
fn proj(raw: &str) -> WsCtx {
    WsCtx::from_parts(Some("s-proj".into()), Some(raw.to_string()))
}

/// 某个写法的**归一键位串**（回显类断言用它）。
///
/// 不把这个串的字面值写死进断言：折叠是 Windows 专属语义，POSIX 上
/// `F:/ws/a` 归一后仍是 `F:/ws/a`，钉字面串会得到一条"Linux 绿、Windows 红"
/// 的平台相关判据。要钉的性质是「回显 == 当前键位」「两种写法 == 同一个键位」。
fn key_of(raw: &str) -> String {
    match proj(raw) {
        WsCtx::Project(ws) => ws,
        _ => panic!("键位夹具没落进 Project 态: {raw:?}"),
    }
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
