//! 双层记忆（㉘·A·fix：全局层 = 用户偏好 / 项目层 = 严格隔离）（自 `tests.rs` 批 F-15
//! 拆出，用例本体逐字未改）：项目写缺省落项目层、全局层要显式 opt-in / 未归属会话不可写
//! 项目记忆 / 读回落全局且项目行优先 / 全局行不可被聊天会话删除 / WsCtx 三段拆分判定。

use super::*;

use plugin_api::serde_json::json;

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
    // 回显取归一键位（Q2），与写入侧落库串同源
    assert_eq!(del["previous_entry"]["workspace"], json!(key_of("F:/ws/a")));
}

#[test]
fn ws_ctx_triage_from_parts() {
    // 有项目根 = Project（键位取**归一形**，Q2）；空串根 = 未归属
    assert!(matches!(
        WsCtx::from_parts(Some("s1".into()), Some("F:/p".into())),
        WsCtx::Project(ws) if ws == key_of("F:/p")
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
