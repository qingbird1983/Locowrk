//! 真源 md 面（自 `tests.rs` 批 F-15 拆出，本刀改名）：写与删直接落
//! `<数据根>/memory/` 下的文件。
//!
//! 这一簇原本叫"人类层**双写**"，测的是 F1 那套"sqlite 已落、md 是同步出去的人读面"。
//! S1 刀③b 之后本插件只有一份存储 ⇒ 原名与那句 `md_warning` 断言都没有可失败的对应物了
//! （字段本身已从实现里消失），所以断言搬去它现在真正管的事：**文件在不在、正文对不对**。
//! 双写时代那条"md 失败只 warn 不破契约"的档就此退役，理由写在 `store.rs` 头注。

use super::*;

use plugin_api::serde_json::json;

#[test]
fn write_and_delete_land_in_the_md_layer() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("md-sync");
    // **Q2 键位归一**：`proj()` 走 `from_parts`，落库与 md 目录取的都是归一后的串，
    // 所以这条判据的目录名必须由 `key_of` 给出同一个串（写死原始写法在 Windows
    // 上会指到一个不存在的目录，Linux 上又恰好指对——那是平台相关的假判据）。
    let ws = key_of(r"F:\proj\MdSync");
    let out = store::memory_write(
        &proj(&ws),
        &json!({ "key": "db.choice", "value": "sqlite", "tags": "pref" }),
    )
    .unwrap();
    // 旧断言是 `md_warning.is_none()`（"同步成功不带回警告"）。那个字段随双写一起消失
    // ⇒ 它已经没有任何可失败的对应物，所以换成这次写入真正的对外读数：落在哪一层、
    // 被算成哪一类。
    assert_eq!(
        out["scope"],
        json!("project"),
        "归属会话缺省落项目层：{out}"
    );
    assert_eq!(out["type"], json!("fact"), "没报类型 = 未分类：{out}");
    let md = dir
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(&ws))
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
            .join(plugin_api::memlayout::project_dir_name(&ws))
            .join("MEMORY.md")
            .exists(),
        "项目索引同步生成"
    );
    // 覆盖写 → md 同步更新
    store::memory_write(
        &proj(&ws),
        &json!({ "key": "db.choice", "value": "postgres" }),
    )
    .unwrap();
    assert!(std::fs::read_to_string(&md).unwrap().contains("postgres"));
    // 删除 → md 消失（索引仍存在但为空行）
    store::memory_delete(&proj(&ws), &json!({ "key": "db.choice" })).unwrap();
    assert!(!md.exists(), "删除同步移除 md");
}
