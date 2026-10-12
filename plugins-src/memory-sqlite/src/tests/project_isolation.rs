//! 项目隔离（㉘）（自 `tests.rs` 批 F-15 拆出，用例本体逐字未改）：同名 key 跨项目共存且
//! 互不可见 / 删除按键位定域且 undo 还原原 workspace / 旧库（仅 key 列）迁移归入未归属。

use super::*;

use plugin_api::serde_json::json;
use rusqlite::Connection;

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
    // 回显取**归一键位**（Q2）：落库串才是补偿层要还原的目标，"这次打进来的写法"不是
    assert_eq!(del["previous_entry"]["workspace"], json!(key_of("F:/ws/a")));

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
