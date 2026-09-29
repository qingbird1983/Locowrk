//! memory 三态用例：正常（roundtrip / undo 还原 / 搜索 / 列举）/ 边界
//! （缺失键 no-op、参数上限、缺字段拒绝）/ 恶意（SQL 注入惰性、LIKE
//! 通配符按字面量）。涉 env（LOCOWRK_DATA_DIR）用例全程持锁串行。

use std::sync::Mutex;

use super::store;
use plugin_api::serde_json::{Value, json};

static ENV_LOCK: Mutex<()> = Mutex::new(());

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

#[test]
fn put_get_roundtrip_and_overwrite_reports_previous() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("roundtrip");

    let out =
        store::memory_put(&json!({ "key": "user.lang", "value": "zh", "tags": "pref" })).unwrap();
    assert_eq!(out["created"], json!(true));
    assert_eq!(out["previous"], Value::Null);

    let got = store::memory_get(&json!({ "key": "user.lang" })).unwrap();
    assert_eq!(got["found"], json!(true));
    assert_eq!(got["value"], json!("zh"));
    assert_eq!(got["tags"], json!("pref"));

    let out2 = store::memory_put(&json!({ "key": "user.lang", "value": "en" })).unwrap();
    assert_eq!(out2["created"], json!(false));
    assert_eq!(out2["previous"]["value"], json!("zh"));
    assert_eq!(
        store::memory_get(&json!({ "key": "user.lang" })).unwrap()["value"],
        json!("en")
    );

    assert_eq!(
        store::memory_get(&json!({ "key": "absent" })).unwrap()["found"],
        json!(false)
    );
}

#[test]
fn put_undo_restores_previous_entry() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("put-undo");

    store::memory_put(&json!({ "key": "k", "value": "old", "tags": "t0" })).unwrap();
    let overwrite =
        store::memory_put(&json!({ "key": "k", "value": "new", "tags": "t1" })).unwrap();

    store::memory_undo(&contract(
        "call-1",
        json!({ "key": "k", "value": "new" }),
        overwrite,
    ))
    .unwrap();

    let got = store::memory_get(&json!({ "key": "k" })).unwrap();
    assert_eq!(got["value"], json!("old"));
    assert_eq!(got["tags"], json!("t0"));
}

#[test]
fn put_undo_without_previous_removes_key() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("put-undo-new");

    let out = store::memory_put(&json!({ "key": "fresh", "value": "v" })).unwrap();
    assert_eq!(out["previous"], Value::Null);
    // 平铺形态（output 字段直接放顶层）也要能 undo
    store::memory_undo(&out).unwrap();
    assert_eq!(
        store::memory_get(&json!({ "key": "fresh" })).unwrap()["found"],
        json!(false)
    );
}

#[test]
fn delete_then_undo_restores_entry() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("delete-undo");

    store::memory_put(&json!({ "key": "note", "value": "keep me", "tags": "x" })).unwrap();
    let del = store::memory_delete(&json!({ "key": "note" })).unwrap();
    assert_eq!(del["deleted"], json!(true));
    assert_eq!(del["previous_entry"]["value"], json!("keep me"));
    assert_eq!(
        store::memory_get(&json!({ "key": "note" })).unwrap()["found"],
        json!(false)
    );

    store::memory_undo(&contract("call-2", json!({ "key": "note" }), del)).unwrap();
    let got = store::memory_get(&json!({ "key": "note" })).unwrap();
    assert_eq!(got["found"], json!(true));
    assert_eq!(got["value"], json!("keep me"));
}

#[test]
fn delete_missing_is_noop_and_undo_of_noop_is_noop() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("delete-missing");

    let del = store::memory_delete(&json!({ "key": "ghost" })).unwrap();
    assert_eq!(del["deleted"], json!(false));
    assert_eq!(del["previous_entry"], Value::Null);

    let undo = store::memory_undo(&contract("call-3", json!({ "key": "ghost" }), del)).unwrap();
    assert_eq!(undo["noop"], json!(true));
}

#[test]
fn list_prefix_limit_offset_and_total() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("list");

    for k in ["a.1", "a.2", "b.1"] {
        store::memory_put(&json!({ "key": k, "value": k })).unwrap();
    }
    let all = store::memory_list(&json!({ "prefix": "a." })).unwrap();
    assert_eq!(all["total"], json!(2));
    assert_eq!(all["entries"].as_array().unwrap().len(), 2);
    assert_eq!(all["entries"][0]["key"], json!("a.1"));

    let page = store::memory_list(&json!({ "prefix": "a.", "limit": 1, "offset": 1 })).unwrap();
    assert_eq!(page["total"], json!(2));
    assert_eq!(page["entries"][0]["key"], json!("a.2"));

    let empty = store::memory_list(&json!({ "prefix": "zzz" })).unwrap();
    assert_eq!(empty["total"], json!(0));
}

#[test]
fn search_is_case_insensitive_and_wildcards_are_literal() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("search");

    store::memory_put(&json!({ "key": "k.one", "value": "100% done_v1" })).unwrap();
    store::memory_put(&json!({ "key": "k.two", "value": "plain text" })).unwrap();

    // % 按字面量：查 "100%" 只命中 k.one，而不是全表
    let hits = store::memory_search(&json!({ "query": "100%" })).unwrap();
    let keys: Vec<&str> = hits["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, vec!["k.one"]);

    // ASCII 大小写不敏感
    let hits = store::memory_search(&json!({ "query": "DONE" })).unwrap();
    assert_eq!(hits["matches"][0]["key"], json!("k.one"));

    // _ 按字面量：只命中带下划线的那条
    let hits = store::memory_search(&json!({ "query": "done_v1" })).unwrap();
    assert_eq!(hits["matches"].as_array().unwrap().len(), 1);
}

#[test]
fn oversized_key_and_value_rejected() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("oversize");

    let long_key = "k".repeat(513);
    assert!(store::memory_put(&json!({ "key": long_key, "value": "v" })).is_err());

    let big_value = "v".repeat(256 * 1024 + 1);
    assert!(store::memory_put(&json!({ "key": "k", "value": big_value })).is_err());

    // 恰好在上限内应当成功（边界内侧）
    let ok_key = "k".repeat(512);
    store::memory_put(&json!({ "key": ok_key, "value": "v" })).unwrap();
}

#[test]
fn sql_injection_in_key_is_inert() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("sqli");

    let evil = "x'); DROP TABLE memories;--";
    store::memory_put(&json!({ "key": evil, "value": "1" })).unwrap();
    assert_eq!(
        store::memory_get(&json!({ "key": evil })).unwrap()["value"],
        json!("1")
    );
    // 表还在，功能不受影响
    let list = store::memory_list(&json!({})).unwrap();
    assert_eq!(list["total"], json!(1));
}

#[test]
fn missing_required_fields_rejected() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("missing-fields");

    assert!(store::memory_put(&json!({ "value": "v" })).is_err());
    assert!(store::memory_put(&json!({ "key": "k" })).is_err());
    assert!(store::memory_get(&json!({})).is_err());
    assert!(store::memory_delete(&json!({})).is_err());
    assert!(store::memory_search(&json!({})).is_err());
}
