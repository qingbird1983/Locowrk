//! memory 三态用例：正常（roundtrip / undo 还原 / 搜索 / 列举）/ 边界
//! （缺失键 no-op、参数上限、缺字段拒绝）/ 恶意（SQL 注入惰性、LIKE
//! 通配符按字面量）/ 项目隔离（㉘：同名 key 各项目互不覆盖互不可见、
//! undo 忠实还原原键位、旧库迁移归入未归属）。涉 env（LOCOWRK_DATA_DIR）
//! 用例全程持锁串行。

use std::sync::Mutex;

use super::WsCtx;
use super::store;
use plugin_api::serde_json::{Value, json};
use rusqlite::Connection;

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

#[test]
fn write_read_roundtrip_and_overwrite_reports_previous() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("write-read");

    let out = store::memory_write(
        &h(),
        &json!({ "key": "user.lang", "value": "zh", "tags": "pref" }),
    )
    .unwrap();
    assert_eq!(out["created"], json!(true));
    assert_eq!(out["previous"], Value::Null);

    let got = store::memory_read(&h(), &json!({ "key": "user.lang" })).unwrap();
    assert_eq!(got["found"], json!(true));
    assert_eq!(got["value"], json!("zh"));
    assert_eq!(got["tags"], json!("pref"));

    let out2 = store::memory_write(&h(), &json!({ "key": "user.lang", "value": "en" })).unwrap();
    assert_eq!(out2["created"], json!(false));
    assert_eq!(out2["previous"]["value"], json!("zh"));
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "user.lang" })).unwrap()["value"],
        json!("en")
    );

    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "absent" })).unwrap()["found"],
        json!(false)
    );
}

#[test]
fn write_undo_restores_previous_entry() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("write-undo");

    store::memory_write(&h(), &json!({ "key": "k", "value": "old", "tags": "t0" })).unwrap();
    let overwrite =
        store::memory_write(&h(), &json!({ "key": "k", "value": "new", "tags": "t1" })).unwrap();

    store::memory_undo(
        &h(),
        &contract("call-1", json!({ "key": "k", "value": "new" }), overwrite),
    )
    .unwrap();

    let got = store::memory_read(&h(), &json!({ "key": "k" })).unwrap();
    assert_eq!(got["value"], json!("old"));
    assert_eq!(got["tags"], json!("t0"));
}

#[test]
fn write_undo_without_previous_removes_key() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("write-undo-new");

    let out = store::memory_write(&h(), &json!({ "key": "fresh", "value": "v" })).unwrap();
    assert_eq!(out["previous"], Value::Null);
    // 平铺形态（output 字段直接放顶层）也要能 undo
    store::memory_undo(&h(), &out).unwrap();
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "fresh" })).unwrap()["found"],
        json!(false)
    );
}

#[test]
fn delete_then_undo_restores_entry() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("delete-undo");

    store::memory_write(
        &h(),
        &json!({ "key": "note", "value": "keep me", "tags": "x" }),
    )
    .unwrap();
    let del = store::memory_delete(&h(), &json!({ "key": "note" })).unwrap();
    assert_eq!(del["deleted"], json!(true));
    assert_eq!(del["previous_entry"]["value"], json!("keep me"));
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "note" })).unwrap()["found"],
        json!(false)
    );

    store::memory_undo(&h(), &contract("call-2", json!({ "key": "note" }), del)).unwrap();
    let got = store::memory_read(&h(), &json!({ "key": "note" })).unwrap();
    assert_eq!(got["found"], json!(true));
    assert_eq!(got["value"], json!("keep me"));
}

#[test]
fn delete_missing_is_noop_and_undo_of_noop_is_noop() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("delete-missing");

    let del = store::memory_delete(&h(), &json!({ "key": "ghost" })).unwrap();
    assert_eq!(del["deleted"], json!(false));
    assert_eq!(del["previous_entry"], Value::Null);

    let undo =
        store::memory_undo(&h(), &contract("call-3", json!({ "key": "ghost" }), del)).unwrap();
    assert_eq!(undo["noop"], json!(true));
}

#[test]
fn list_prefix_limit_offset_and_total() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("list");

    for k in ["a.1", "a.2", "b.1"] {
        store::memory_write(&h(), &json!({ "key": k, "value": k })).unwrap();
    }
    let all = store::memory_list(&h(), &json!({ "prefix": "a." })).unwrap();
    assert_eq!(all["total"], json!(2));
    assert_eq!(all["entries"].as_array().unwrap().len(), 2);
    assert_eq!(all["entries"][0]["key"], json!("a.1"));

    let page =
        store::memory_list(&h(), &json!({ "prefix": "a.", "limit": 1, "offset": 1 })).unwrap();
    assert_eq!(page["total"], json!(2));
    assert_eq!(page["entries"][0]["key"], json!("a.2"));

    let empty = store::memory_list(&h(), &json!({ "prefix": "zzz" })).unwrap();
    assert_eq!(empty["total"], json!(0));
}

#[test]
fn search_is_case_insensitive_and_wildcards_are_literal() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("search");

    store::memory_write(&h(), &json!({ "key": "k.one", "value": "100% done_v1" })).unwrap();
    store::memory_write(&h(), &json!({ "key": "k.two", "value": "plain text" })).unwrap();

    // % 按字面量：查 "100%" 只命中 k.one，而不是全表
    let hits = store::memory_search(&h(), &json!({ "query": "100%" })).unwrap();
    let keys: Vec<&str> = hits["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, vec!["k.one"]);

    // ASCII 大小写不敏感
    let hits = store::memory_search(&h(), &json!({ "query": "DONE" })).unwrap();
    assert_eq!(hits["matches"][0]["key"], json!("k.one"));

    // _ 按字面量：只命中带下划线的那条
    let hits = store::memory_search(&h(), &json!({ "query": "done_v1" })).unwrap();
    assert_eq!(hits["matches"].as_array().unwrap().len(), 1);
}

#[test]
fn oversized_key_and_value_rejected() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("oversize");

    let long_key = "k".repeat(513);
    assert!(store::memory_write(&h(), &json!({ "key": long_key, "value": "v" })).is_err());

    let big_value = "v".repeat(256 * 1024 + 1);
    assert!(store::memory_write(&h(), &json!({ "key": "k", "value": big_value })).is_err());

    // 恰好在上限内应当成功（边界内侧）
    let ok_key = "k".repeat(512);
    store::memory_write(&h(), &json!({ "key": ok_key, "value": "v" })).unwrap();
}

#[test]
fn sql_injection_in_key_is_inert() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("sqli");

    let evil = "x'); DROP TABLE memories;--";
    store::memory_write(&h(), &json!({ "key": evil, "value": "1" })).unwrap();
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": evil })).unwrap()["value"],
        json!("1")
    );
    // 表还在，功能不受影响
    let list = store::memory_list(&h(), &json!({})).unwrap();
    assert_eq!(list["total"], json!(1));
}

#[test]
fn missing_required_fields_rejected() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("missing-fields");

    assert!(store::memory_write(&h(), &json!({ "value": "v" })).is_err());
    assert!(store::memory_write(&h(), &json!({ "key": "k" })).is_err());
    assert!(store::memory_read(&h(), &json!({})).is_err());
    assert!(store::memory_delete(&h(), &json!({})).is_err());
    assert!(store::memory_search(&h(), &json!({})).is_err());
}

// ---- 项目隔离（㉘）--------------------------------------------------------

#[test]
fn same_key_coexists_across_projects_and_is_invisible_cross_project() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("project-scope");

    store::memory_write(&proj("F:/ws/a"), &json!({ "key": "stack", "value": "vue" })).unwrap();
    store::memory_write(
        &proj("F:/ws/b"),
        &json!({ "key": "stack", "value": "rust" }),
    )
    .unwrap();
    // 未归属键位同 key 再一条，三方互不覆盖
    store::memory_write(&h(), &json!({ "key": "stack", "value": "plain" })).unwrap();

    assert_eq!(
        store::memory_read(&proj("F:/ws/a"), &json!({ "key": "stack" })).unwrap()["value"],
        json!("vue")
    );
    assert_eq!(
        store::memory_read(&proj("F:/ws/b"), &json!({ "key": "stack" })).unwrap()["value"],
        json!("rust")
    );
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "stack" })).unwrap()["value"],
        json!("plain")
    );

    // 跨项目不可见：a 看不到 b 的数据（全局行除外——plain 是 '' 键位 =
    // 全局层，a 能看到；双层纪律 §4.6）
    let hits = store::memory_search(&proj("F:/ws/a"), &json!({ "query": "rust" })).unwrap();
    assert_eq!(hits["matches"].as_array().unwrap().len(), 0);
    let hits = store::memory_search(&proj("F:/ws/a"), &json!({ "query": "plain" })).unwrap();
    assert_eq!(hits["matches"][0]["scope"], json!("global"));
    // b 的列举 = b 项目行 ∪ 全局行，同名 key 两条、各带层标
    let list = store::memory_list(&proj("F:/ws/b"), &json!({})).unwrap();
    assert_eq!(list["total"], json!(2));
    let mut vals: Vec<&str> = list["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["value"].as_str().unwrap())
        .collect();
    vals.sort();
    assert_eq!(vals, vec!["plain", "rust"]);
    let mut scopes: Vec<&str> = list["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["scope"].as_str().unwrap())
        .collect();
    scopes.sort();
    assert_eq!(scopes, vec!["global", "project"]);
}

#[test]
fn delete_is_scoped_and_undo_restores_original_workspace() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("project-undo");

    store::memory_write(&proj("F:/ws/a"), &json!({ "key": "k", "value": "a-val" })).unwrap();
    let del = store::memory_delete(&proj("F:/ws/a"), &json!({ "key": "k" })).unwrap();
    assert_eq!(del["previous_entry"]["workspace"], json!("F:/ws/a"));

    // 补偿调用发生在别的键位（极端时序）：快照带 workspace → 忠实还原
    // 到原项目，而不是当前调用键位
    store::memory_undo(&h(), &contract("call-4", json!({ "key": "k" }), del)).unwrap();
    assert_eq!(
        store::memory_read(&proj("F:/ws/a"), &json!({ "key": "k" })).unwrap()["value"],
        json!("a-val")
    );
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "k" })).unwrap()["found"],
        json!(false)
    );
}

#[test]
fn legacy_key_only_table_migrates_into_unfiled_scope() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("legacy-migrate");
    let db = dir.join("memory.sqlite3");

    // 手工造一张 v2.54 时代的旧表（key 单主键、无 workspace/source）
    {
        std::fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                 key        TEXT PRIMARY KEY,
                 value      TEXT NOT NULL,
                 tags       TEXT NOT NULL DEFAULT '',
                 updated_at INTEGER NOT NULL
             );
             INSERT INTO memories (key, value, tags, updated_at)
                 VALUES ('old.k', 'old-value', '', 123);",
        )
        .unwrap();
    }

    // 首次访问触发迁移：旧行归入未归属键位，读写照常
    let got = store::memory_read(&h(), &json!({ "key": "old.k" })).unwrap();
    assert_eq!(got["value"], json!("old-value"));
    // 迁移后新键位生效：同 key 在项目下是另一条
    store::memory_write(
        &proj("F:/ws/a"),
        &json!({ "key": "old.k", "value": "project-value" }),
    )
    .unwrap();
    assert_eq!(
        store::memory_read(&proj("F:/ws/a"), &json!({ "key": "old.k" })).unwrap()["value"],
        json!("project-value")
    );
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "old.k" })).unwrap()["value"],
        json!("old-value")
    );
}

// ---- 双层记忆（㉘·A·fix：全局层 = 用户偏好 / 项目层 = 严格隔离）----------

#[test]
fn project_write_defaults_to_project_and_global_scope_needs_opt_in() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("two-tier-write");

    let out = store::memory_write(
        &proj("F:/ws/a"),
        &json!({ "key": "arch.stack", "value": "vue" }),
    )
    .unwrap();
    assert_eq!(out["scope"], json!("project"));

    let out = store::memory_write(
        &proj("F:/ws/a"),
        &json!({ "key": "user.lang", "value": "zh", "scope": "global" }),
    )
    .unwrap();
    assert_eq!(out["scope"], json!("global"));

    // 非法 scope 拒绝
    assert!(
        store::memory_write(
            &proj("F:/ws/a"),
            &json!({ "key": "k", "value": "v", "scope": "universe" })
        )
        .is_err()
    );
}

#[test]
fn unattributed_session_cannot_write_project_memory() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("two-tier-unattributed");

    // 缺省写拒绝（没有项目的"项目记忆"不许落地污染全局层）
    let err = store::memory_write(
        &WsCtx::Unattributed,
        &json!({ "key": "note", "value": "v" }),
    )
    .unwrap_err();
    assert!(err.contains("scope=\"global\""), "{err}");

    // 显式升格为全局偏好：允许
    store::memory_write(
        &WsCtx::Unattributed,
        &json!({ "key": "user.pref", "value": "likes concise answers", "scope": "global" }),
    )
    .unwrap();
    assert_eq!(
        store::memory_read(&WsCtx::Unattributed, &json!({ "key": "user.pref" })).unwrap()["scope"],
        json!("global")
    );

    // 未归属会话读不到别的项目行
    store::memory_write(
        &proj("F:/ws/a"),
        &json!({ "key": "arch.note", "value": "x" }),
    )
    .unwrap();
    assert_eq!(
        store::memory_read(&WsCtx::Unattributed, &json!({ "key": "arch.note" })).unwrap()["found"],
        json!(false)
    );
}

#[test]
fn read_falls_back_to_global_and_project_row_wins() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("two-tier-read");

    // 全局层有 user.lang；项目层同 key 另一条 → 项目行优先、带层标
    store::memory_write(&h(), &json!({ "key": "user.lang", "value": "zh" })).unwrap();
    store::memory_write(
        &proj("F:/ws/a"),
        &json!({ "key": "user.lang", "value": "en" }),
    )
    .unwrap();
    let got = store::memory_read(&proj("F:/ws/a"), &json!({ "key": "user.lang" })).unwrap();
    assert_eq!(got["value"], json!("en"));
    assert_eq!(got["scope"], json!("project"));

    // 项目层没有的 key 回落全局层、带层标
    store::memory_write(&h(), &json!({ "key": "user.editor", "value": "vscode" })).unwrap();
    let got = store::memory_read(&proj("F:/ws/a"), &json!({ "key": "user.editor" })).unwrap();
    assert_eq!(got["value"], json!("vscode"));
    assert_eq!(got["scope"], json!("global"));
    // 其他项目同样读到该全局偏好
    assert_eq!(
        store::memory_read(&proj("F:/ws/b"), &json!({ "key": "user.editor" })).unwrap()["scope"],
        json!("global")
    );
}

#[test]
fn global_rows_not_deletable_from_chat_sessions() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("two-tier-delete");

    store::memory_write(&h(), &json!({ "key": "user.pref", "value": "keep" })).unwrap();

    // 归属会话：项目里没有、全局有 → 拒绝并指向管理页
    let err = store::memory_delete(&proj("F:/ws/a"), &json!({ "key": "user.pref" })).unwrap_err();
    assert!(err.contains("memory management page"), "{err}");

    // 未归属会话：同样禁删全局
    assert!(store::memory_delete(&WsCtx::Unattributed, &json!({ "key": "user.pref" })).is_err());

    // headless（'' 单一宇宙）保持旧行为：可删、可 undo 还原
    let del = store::memory_delete(&h(), &json!({ "key": "user.pref" })).unwrap();
    assert_eq!(del["deleted"], json!(true));
    store::memory_undo(
        &h(),
        &contract("call-9", json!({ "key": "user.pref" }), del),
    )
    .unwrap();
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "user.pref" })).unwrap()["found"],
        json!(true)
    );

    // 项目行删除不受影响（全局层没有同名 key 时正常走项目键位）
    store::memory_write(&proj("F:/ws/a"), &json!({ "key": "note", "value": "mine" })).unwrap();
    let del = store::memory_delete(&proj("F:/ws/a"), &json!({ "key": "note" })).unwrap();
    assert_eq!(del["deleted"], json!(true));
    assert_eq!(del["previous_entry"]["workspace"], json!("F:/ws/a"));
}

#[test]
fn ws_ctx_triage_from_parts() {
    use super::WsCtx;
    // 有项目根 = Project；空串根 = 未归属
    assert!(matches!(
        WsCtx::from_parts(Some("s1".into()), Some("F:/p".into())),
        WsCtx::Project(ws) if ws == "F:/p"
    ));
    assert!(matches!(
        WsCtx::from_parts(Some("s1".into()), None),
        WsCtx::Unattributed
    ));
    assert!(matches!(
        WsCtx::from_parts(Some("s1".into()), Some(String::new())),
        WsCtx::Unattributed
    ));
    // 无会话身份 = headless 等价（一次性进程 / 测试装配）
    assert!(matches!(WsCtx::from_parts(None, None), WsCtx::Headless));
    assert!(matches!(
        WsCtx::from_parts(None, Some(String::new())),
        WsCtx::Headless
    ));
}
