//! 键位归一（MR 稿 §6 · 队列 Q2）在**插件这一侧**的判据。
//!
//! 主轴不是"`wskey::normalize` 算得对不对"（那归它自己的单测管），而是
//! **「同一个项目的两种写法，在库里必须只有一行」**——这件事只有驱动 `store`
//! 的生产本体才判得出来：写侧走 `write_scope`、读侧走 `scope()`，两侧都取自
//! `WsCtx`，而归一就发生在 `WsCtx::from_parts` 里。夹具一律经 `proj()`
//! （= `from_parts`）构造，不许手搭 `WsCtx::Project` 绕过它。
//!
//! 平台档说明：尾分隔符与 `.` 的漂移**跨平台成立**；大小写折叠与斜杠方向
//! **只在 Windows 成立**——POSIX 上大小写敏感，把它们合起来是数据损坏，
//! 所以那两档写成 `#[cfg(windows)]`，并在 POSIX 侧留一条"不许合并"的反向判据。

use super::*;

use plugin_api::serde_json::json;

/// 跨平台成立的漂移：尾部多余分隔符（粘贴路径的常见形态）。
const SPELL_A: &str = "F:/ws/a";
const SPELL_TRAILING: &str = "F:/ws/a/";

#[test]
fn two_spellings_of_one_project_hold_exactly_one_row() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("keyside-one-row");
    store::memory_write(&proj(SPELL_A), &json!({ "key": "stack", "value": "vue" })).unwrap();
    // 换一种写法读：必须是**同一条**，而不是"未命中"（双键位就是这个形状失效的）
    let got = store::memory_read(&proj(SPELL_TRAILING), &json!({ "key": "stack" })).unwrap();
    assert_eq!(
        got["found"],
        json!(true),
        "尾斜杠写法读不到 ⇒ 劈成了两个键位"
    );
    assert_eq!(got["value"], json!("vue"));
    assert_eq!(
        got["scope"],
        json!("project"),
        "项目层身份不许因写法而变成全局"
    );

    // 再换写法**覆盖写**：主键冲突才说明两侧同一个键位（否则第二条会新插一行）
    store::memory_write(
        &proj(SPELL_TRAILING),
        &json!({ "key": "stack", "value": "react" }),
    )
    .unwrap();
    let list = store::memory_list(&proj(SPELL_A), &json!({})).unwrap();
    assert_eq!(list["total"], json!(1), "两种写法不许写出两行: {list}");
    assert_eq!(
        store::memory_read(&proj(SPELL_A), &json!({ "key": "stack" })).unwrap()["value"],
        json!("react"),
        "覆盖写落在同一行上"
    );
}

#[test]
fn search_and_list_reach_the_row_written_with_another_spelling() {
    // 查询面与写入面同键位才算归一成立（只测 read 会漏掉 `memory_search`
    // 与 `memory_list` 这两条独立 SQL）。
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("keyside-query-faces");
    store::memory_write(
        &proj(SPELL_A),
        &json!({ "key": "build.cmd", "value": "pnpm tauri dev", "tags": "tauri" }),
    )
    .unwrap();
    let hits = store::memory_search(&proj(SPELL_TRAILING), &json!({ "query": "tauri" })).unwrap();
    assert_eq!(hits["matches"].as_array().unwrap().len(), 1);
    let listed = store::memory_list(&proj(SPELL_TRAILING), &json!({ "prefix": "build." })).unwrap();
    assert_eq!(listed["total"], json!(1));
}

#[test]
fn undo_echoes_the_normalized_keyside_not_the_spelling_typed_in() {
    // `previous_entry.workspace` 是补偿层与审计要读的回显；它必须等于**落库那个串**
    // （归一形），而不是"模型这次打的那串"——否则 undo 会把行还原到一个
    // 没归一的旧写法键位上，双键位又被造回来。
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("keyside-undo");
    store::memory_write(&proj(SPELL_A), &json!({ "key": "note", "value": "v1" })).unwrap();
    let del = store::memory_delete(&proj(SPELL_TRAILING), &json!({ "key": "note" })).unwrap();
    assert_eq!(del["deleted"], json!(true));
    let echoed = del["previous_entry"]["workspace"].as_str().unwrap();
    assert_eq!(echoed, key_of(SPELL_A), "回显必须是归一键位");
    assert_eq!(echoed, key_of(SPELL_TRAILING), "两种写法归一到同一个回显串");
    store::memory_undo(
        &proj(SPELL_TRAILING),
        &contract("call-1", json!({ "key": "note" }), del.clone()),
    )
    .unwrap();
    assert_eq!(
        store::memory_list(&proj(SPELL_A), &json!({})).unwrap()["total"],
        json!(1),
        "undo 落回同一个键位"
    );
}

#[test]
fn a_blank_workspace_never_becomes_a_project_keyside() {
    // `"   "` 折叠后为空 = **未归属**，不许当成"某个名字叫空白的项目"落进
    // 项目键位（那样它会以不可见的形态污染项目层）。
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("keyside-blank");
    let err =
        store::memory_write(&proj("   "), &json!({ "key": "note", "value": "v" })).unwrap_err();
    assert!(
        err.contains("no project"),
        "空白串该按未归属拒绝项目写，实际: {err}"
    );
    // 而显式 global 仍然可写（未归属会话的全局偏好路径不受本刀影响）
    store::memory_write(
        &proj("   "),
        &json!({ "key": "user.pref", "value": "pnpm", "scope": "global" }),
    )
    .unwrap();
    assert_eq!(
        store::memory_read(&h(), &json!({ "key": "user.pref" })).unwrap()["found"],
        json!(true),
        "全局层键位仍是 ''"
    );
}

#[cfg(windows)]
#[test]
fn windows_spelling_variants_all_land_on_one_row() {
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("keyside-win");
    let variants = [
        r"F:\Warehouse\TITY",
        r"f:\warehouse\tity",
        r"F:/Warehouse/TITY/",
        r"F:\WAREHOUSE\tity\",
    ];
    store::memory_write(
        &proj(variants[0]),
        &json!({ "key": "stack", "value": "brick" }),
    )
    .unwrap();
    for v in &variants[1..] {
        let got = store::memory_read(&proj(v), &json!({ "key": "stack" })).unwrap();
        assert_eq!(got["found"], json!(true), "写法 {v:?} 读不到同一条");
        assert_eq!(
            store::memory_list(&proj(v), &json!({})).unwrap()["total"],
            json!(1),
            "写法 {v:?} 造出了第二行"
        );
    }
}

#[cfg(not(windows))]
#[test]
fn posix_keeps_two_spellings_apart_because_they_are_two_directories() {
    // 反向判据：Linux CI 上必须**不合并**大小写。合并它 = 把两个真不同的项目
    // 的记忆写进同一个主键，那是数据损坏而不是归一。
    let _g = ENV_LOCK.lock().unwrap();
    temp_data_dir("keyside-posix");
    store::memory_write(
        &proj("/ws/ProjA"),
        &json!({ "key": "stack", "value": "vue" }),
    )
    .unwrap();
    let got = store::memory_read(&proj("/ws/proja"), &json!({ "key": "stack" })).unwrap();
    assert_eq!(got["found"], json!(false), "POSIX 不许折叠大小写");
}
