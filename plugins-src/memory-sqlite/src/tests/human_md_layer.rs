//! 人类层双写（F1，《Memory File-ization Design》§3）（自 `tests.rs` 批 F-15 拆出，
//! 用例本体逐字未改）：写与删同步 memory md 文件。

use super::*;

use plugin_api::serde_json::json;

// ---- F1 人类层双写（《Memory File-ization Design》§3）----

#[test]
fn write_and_delete_sync_md_human_layer() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("md-sync");
    let ws = r"F:\proj\MdSync";
    let out = store::memory_write(
        &proj(ws),
        &json!({ "key": "db.choice", "value": "sqlite", "tags": "pref" }),
    )
    .unwrap();
    assert!(out.get("md_warning").is_none(), "双写成功不带回警告：{out}");
    let md = dir
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(ws))
        .join("facts")
        .join("db.choice.md");
    let text = std::fs::read_to_string(&md).expect("md 文件在场");
    assert!(
        text.contains("name: \"db.choice\""),
        "frontmatter 身份：{text}"
    );
    assert!(text.contains("sqlite"), "正文在场");
    assert!(
        dir.join("memory")
            .join("projects")
            .join(plugin_api::memlayout::project_dir_name(ws))
            .join("MEMORY.md")
            .exists(),
        "项目索引同步生成"
    );
    // 覆盖写 → md 同步更新
    store::memory_write(
        &proj(ws),
        &json!({ "key": "db.choice", "value": "postgres" }),
    )
    .unwrap();
    assert!(std::fs::read_to_string(&md).unwrap().contains("postgres"));
    // 删除 → md 消失（索引仍存在但为空行）
    store::memory_delete(&proj(ws), &json!({ "key": "db.choice" })).unwrap();
    assert!(!md.exists(), "删除同步移除 md");
}
