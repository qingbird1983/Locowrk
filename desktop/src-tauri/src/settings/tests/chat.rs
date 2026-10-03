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
    // M0 观测缺省：显示开 + deepseek-flash 空闲价（0.02 / 1.0 元每百万）
    assert!(v.cache_stats);
    assert_eq!(
        v.cache_hit_price,
        super::super::chat::CACHE_HIT_PRICE_DEFAULT
    );
    assert_eq!(
        v.cache_miss_price,
        super::super::chat::CACHE_MISS_PRICE_DEFAULT
    );
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
                auto_title: Some(false),
                cache_stats: Some(false),
                cache_hit_price: Some(0.01),
                cache_miss_price: Some(2.0),
            }),
            None,
        )
        .unwrap()
        .chat;
    assert_eq!(v.content_max_width, Some(1000));
    assert!(v.expand_tool_process);
    assert!(!v.enter_to_send);
    assert_eq!(v.usage_readout, "remaining");
    assert!(!v.auto_title, "显式关闭生效");
    assert!(!v.cache_stats);
    assert_eq!(v.cache_hit_price, 0.01);
    assert_eq!(v.cache_miss_price, 2.0);
    // 磁盘往返
    let v = SettingsStore::open(&root).unwrap().view().chat;
    assert_eq!(v.content_max_width, Some(1000));
    assert_eq!(v.usage_readout, "remaining");
    assert_eq!(v.cache_hit_price, 0.01);
    assert_eq!(v.cache_miss_price, 2.0);
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
                auto_title: None,
                cache_stats: None,
                cache_hit_price: None,
                cache_miss_price: None,
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
                auto_title: None,
                cache_stats: None,
                cache_hit_price: None,
                cache_miss_price: None,
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
                auto_title: None,
                cache_stats: None,
                cache_hit_price: None,
                cache_miss_price: None,
            }),
            None,
        )
        .is_err());
    // 失败不污染内存态
    assert_eq!(store.view().chat.content_max_width, None);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn chat_rejects_invalid_price_and_keeps_state() {
    let root = temp_root("chat-price-invalid");
    let store = SettingsStore::open(&root).unwrap();
    let reject = |hit: Option<f64>, miss: Option<f64>| {
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
                None,
                None,
                Some(ChatUpdate {
                    content_max_width: None,
                    expand_tool_process: None,
                    enter_to_send: None,
                    usage_readout: None,
                    auto_title: None,
                    cache_stats: None,
                    cache_hit_price: hit,
                    cache_miss_price: miss,
                }),
                None,
            )
            .is_err()
    };
    assert!(reject(Some(-0.01), None), "负单价拒绝");
    assert!(reject(None, Some(10_001.0)), "超上限拒绝");
    // 拒绝不污染内存态：单价仍是缺省
    let v = store.view().chat;
    assert_eq!(
        v.cache_hit_price,
        super::super::chat::CACHE_HIT_PRICE_DEFAULT
    );
    assert_eq!(
        v.cache_miss_price,
        super::super::chat::CACHE_MISS_PRICE_DEFAULT
    );
    // 0 合法（免单价场景）；上限值合法
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
            None,
            None,
            Some(ChatUpdate {
                content_max_width: None,
                expand_tool_process: None,
                enter_to_send: None,
                usage_readout: None,
                auto_title: None,
                cache_stats: None,
                cache_hit_price: Some(0.0),
                cache_miss_price: Some(super::super::chat::PRICE_MAX),
            }),
            None,
        )
        .unwrap();
    std::fs::remove_dir_all(&root).ok();
}
