//! 对话行为区设置测试（UI shell 文档 §3.2 ChatSection / §2.7 列宽）：缺省
//! 与旧文件兼容 / 更新往返 / 非法值拒绝（宽度出界、读数口径未知）。

use super::*;

#[test]
fn chat_defaults_and_legacy_compat() {
    let root = temp_root("chat-defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().chat;
    assert_eq!(v.content_max_width, None); // None = 前端按缺省 760 渲染
    assert!(!v.collapse_tool_process);
    assert!(v.enter_to_send);
    assert_eq!(v.usage_readout, "used");
    // M0 观测缺省：显示开（v2.137 两档单价退役为幻影键）
    assert!(v.cache_stats);
    // 长消息折叠缺省：开 + 5 行
    assert!(v.fold_long_messages);
    assert_eq!(v.fold_lines, 5);
    // 旧 settings.json 无 chat 区 → 字段级缺省，零迁移
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().chat;
    assert_eq!(v.content_max_width, None);
    assert!(v.enter_to_send);
    // v4.20：旧 expandToolProcess 键退役 = 幻影键——旧文件读得进、新键走缺省
    std::fs::write(
        root.join("settings.json"),
        r#"{"chat": {"expandToolProcess": true}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().chat;
    assert!(!v.collapse_tool_process, "退役键幻影化：折叠开关走缺省关");
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
            Some(ChatUpdate {
                content_max_width: Some(1000),
                collapse_tool_process: Some(true),
                enter_to_send: Some(false),
                usage_readout: Some("remaining".into()),
                auto_title: Some(false),
                cache_stats: Some(false),
                fold_long_messages: Some(false),
                fold_lines: Some(12),
            }),
            None,
            None,
            None,
        )
        .unwrap()
        .chat;
    assert_eq!(v.content_max_width, Some(1000));
    assert!(v.collapse_tool_process);
    assert!(!v.enter_to_send);
    assert_eq!(v.usage_readout, "remaining");
    assert!(!v.auto_title, "显式关闭生效");
    assert!(!v.cache_stats);
    assert!(!v.fold_long_messages, "显式关闭生效");
    assert_eq!(v.fold_lines, 12);
    // 磁盘往返
    let v = SettingsStore::open(&root).unwrap().view().chat;
    assert_eq!(v.content_max_width, Some(1000));
    assert_eq!(v.usage_readout, "remaining");
    assert_eq!(v.fold_lines, 12);
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
            Some(ChatUpdate {
                content_max_width: None,
                collapse_tool_process: None,
                enter_to_send: Some(true),
                usage_readout: None,
                auto_title: None,
                cache_stats: None,
                fold_long_messages: None,
                fold_lines: None,
            }),
            None,
            None,
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
            Some(ChatUpdate {
                content_max_width: Some(200),
                collapse_tool_process: None,
                enter_to_send: None,
                usage_readout: None,
                auto_title: None,
                cache_stats: None,
                fold_long_messages: None,
                fold_lines: None,
            }),
            None,
            None,
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
            Some(ChatUpdate {
                content_max_width: None,
                collapse_tool_process: None,
                enter_to_send: None,
                usage_readout: Some("total".into()),
                auto_title: None,
                cache_stats: None,
                fold_long_messages: None,
                fold_lines: None,
            }),
            None,
            None,
            None,
        )
        .is_err());
    // 失败不污染内存态
    assert_eq!(store.view().chat.content_max_width, None);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn chat_rejects_bad_fold_lines_and_keeps_state() {
    let root = temp_root("chat-fold-invalid");
    let store = SettingsStore::open(&root).unwrap();
    let reject = |n: Option<u32>| {
        store
            .update(
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
                    collapse_tool_process: None,
                    enter_to_send: None,
                    usage_readout: None,
                    auto_title: None,
                    cache_stats: None,
                    fold_long_messages: None,
                    fold_lines: n,
                }),
                None,
                None,
                None,
            )
            .is_err()
    };
    assert!(reject(Some(2)), "低于下限拒绝");
    assert!(reject(Some(21)), "高于上限拒绝");
    // 拒绝不污染内存态：缺省仍是 5
    assert_eq!(store.view().chat.fold_lines, 5);
    // 边界值合法
    assert!(!reject(Some(3)));
    assert!(!reject(Some(20)));
    assert_eq!(store.view().chat.fold_lines, 20);
    std::fs::remove_dir_all(&root).ok();
}
