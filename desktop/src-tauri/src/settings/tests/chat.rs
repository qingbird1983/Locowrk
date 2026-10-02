//! 对话行为区设置测试（UI shell 文档 §3.2 ChatSection / §2.7 列宽）：缺省
//! 与旧文件兼容 / 更新往返 / 非法值拒绝（宽度出界、读数口径未知）。

use super::*;

#[test]
fn chat_defaults_and_legacy_compat() {
    let root = temp_root("chat-defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().chat;
    assert_eq!(v.content_max_width, None); // None = 前端按缺省 760 渲染
    assert!(!v.expand_tool_process);
    assert!(v.enter_to_send);
    assert_eq!(v.usage_readout, "used");
    // 旧 settings.json 无 chat 区 → 字段级缺省，零迁移
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().chat;
    assert_eq!(v.content_max_width, None);
    assert!(v.enter_to_send);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn chat_update_persists_and_roundtrips() {
    let root = temp_root("chat-update");
    let store = SettingsStore::open(&root).unwrap();
    let v = store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ChatUpdate {
                content_max_width: Some(1000),
                expand_tool_process: Some(true),
                enter_to_send: Some(false),
                usage_readout: Some("remaining".into()),
            }),
            None,
        )
        .unwrap()
        .chat;
    assert_eq!(v.content_max_width, Some(1000));
    assert!(v.expand_tool_process);
    assert!(!v.enter_to_send);
    assert_eq!(v.usage_readout, "remaining");
    // 磁盘往返
    let v = SettingsStore::open(&root).unwrap().view().chat;
    assert_eq!(v.content_max_width, Some(1000));
    assert_eq!(v.usage_readout, "remaining");
    // 部分更新：None 字段保持不变
    let v = store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ChatUpdate {
                content_max_width: None,
                expand_tool_process: None,
                enter_to_send: Some(true),
                usage_readout: None,
            }),
            None,
        )
        .unwrap()
        .chat;
    assert_eq!(v.content_max_width, Some(1000));
    assert!(v.enter_to_send);
    assert_eq!(v.usage_readout, "remaining");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn chat_rejects_out_of_range_width_and_unknown_readout() {
    let root = temp_root("chat-invalid");
    let store = SettingsStore::open(&root).unwrap();
    assert!(store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ChatUpdate {
                content_max_width: Some(200),
                expand_tool_process: None,
                enter_to_send: None,
                usage_readout: None,
            }),
            None,
        )
        .is_err());
    assert!(store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ChatUpdate {
                content_max_width: None,
                expand_tool_process: None,
                enter_to_send: None,
                usage_readout: Some("total".into()),
            }),
            None,
        )
        .is_err());
    // 失败不污染内存态
    assert_eq!(store.view().chat.content_max_width, None);
    std::fs::remove_dir_all(&root).ok();
}
