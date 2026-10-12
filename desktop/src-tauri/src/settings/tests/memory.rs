//! 记忆注入区（⑪，Context and Memory.md §6.2）设置测试：缺省值 /
//! 旧文件兼容 / 部分更新语义 / 校验拒绝。

use super::*;

// ---- 记忆注入区（⑪，Context and Memory.md §6.2） ----

#[test]
fn memory_defaults_legacy_compat_and_validation() {
    let root = temp_root("memory");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：开 / 注入 5 条
    let v = store.view().memory;
    assert!(v.enabled);
    assert_eq!(v.top_k, 5);
    // memory_inject_config 桥接（MemoryInject 共享锁内容物）
    let cfg = store.memory_inject_config();
    assert!(cfg.enabled);
    assert_eq!(cfg.top_k, 5);
    // 旧 settings.json 无 memory 字段 → 字段级缺省，不解析失败
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().memory;
    assert!(v.enabled);
    assert_eq!(v.top_k, 5);
    // 更新 + 磁盘往返
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                enabled: Some(false),
                top_k: Some(3),
                organize_pacing: None,
                organize_chunk_tokens: None,
                memory_manager_model: None,
                memory_manager_provider: None,
                tail_strategy: None,
                tail_window_turns: None,
                collapse_enabled: None,
                collapse_threshold_pct: None,
                files: None,
                repo_context: None,
                instruction_files: None,
                instruction_global_kib: None,
                instruction_project_kib: None,
                fact_cards: None,
                fact_card_max: None,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(!store.view().memory.enabled);
    assert_eq!(store.view().memory.top_k, 3);
    let cfg = store.memory_inject_config();
    assert!(!cfg.enabled);
    assert_eq!(cfg.top_k, 3);
    assert_eq!(SettingsStore::open(&root).unwrap().view().memory.top_k, 3);
    // 部分更新：只给 top_k，enabled 保持
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                top_k: Some(8),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(!store.view().memory.enabled);
    assert_eq!(store.view().memory.top_k, 8);
    // 校验：top_k 出界拒绝且不污染内存态
    for bad in [0, 21] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                None,
                None,
                Some(MemoryUpdate {
                    top_k: Some(bad),
                    ..Default::default()
                }),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .is_err());
    }
    assert_eq!(store.view().memory.top_k, 8);
    std::fs::remove_dir_all(&root).ok();
}

// ---- M2 记忆树整理三键（树 v2 §4.3/§5.2/§15）：节奏 / 增量块 / 记忆管理器 ----

#[test]
fn organize_keys_roundtrip_validation_and_bridge() {
    let root = temp_root("memory-organize");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：batch / 50K / 记忆管理器未配（同主模型）
    let v = store.view().memory;
    assert_eq!(v.organize_pacing, "batch");
    assert_eq!(v.organize_chunk_tokens, 50_000);
    assert!(v.memory_manager_model.is_none());
    assert!(v.memory_manager_provider.is_none());
    // 更新 + 磁盘往返（重开读回 = 永久保存的硬证明）
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                enabled: None,
                top_k: None,
                organize_pacing: Some("incremental".into()),
                organize_chunk_tokens: Some(80_000),
                memory_manager_model: Some("deepseek-flash".into()),
                memory_manager_provider: Some("p1".into()),
                tail_strategy: None,
                tail_window_turns: None,
                collapse_enabled: None,
                collapse_threshold_pct: None,
                files: None,
                repo_context: None,
                instruction_files: None,
                instruction_global_kib: None,
                instruction_project_kib: None,
                fact_cards: None,
                fact_card_max: None,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    drop(store);
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().memory;
    assert_eq!(v.organize_pacing, "incremental");
    assert_eq!(v.organize_chunk_tokens, 80_000);
    assert_eq!(v.memory_manager_model.as_deref(), Some("deepseek-flash"));
    assert_eq!(v.memory_manager_provider.as_deref(), Some("p1"));
    // 校验：节奏白名单 / 增量块区间
    let err = store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                organize_pacing: Some("weekly".into()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
    assert!(err.contains("整理节奏"), "{err}");
    let err = store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                organize_chunk_tokens: Some(1_000),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
    assert!(err.contains("增量块"), "{err}");
    // 记忆管理器模型传空串 = 清除（回落同主模型）
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                memory_manager_model: Some(String::new()),
                memory_manager_provider: Some(String::new()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(store.view().memory.memory_manager_model.is_none());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn tail_strategy_window_and_collapse_roundtrip_with_validation() {
    let root = temp_root("tail");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省 = A 边界结算 / 窗口 5 / collapse 关 / 阈值 80
    let v = store.view().memory;
    assert_eq!(v.tail_strategy, "boundary");
    assert_eq!(v.tail_window_turns, 5);
    assert!(!v.collapse_enabled);
    assert_eq!(v.collapse_threshold_pct, 80);

    // 往返：B 固定窗口 + 窗口 12 + collapse 开 + 阈值 90
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                tail_strategy: Some("window".into()),
                tail_window_turns: Some(12),
                collapse_enabled: Some(true),
                collapse_threshold_pct: Some(90),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let v = store.view().memory;
    assert_eq!(v.tail_strategy, "window");
    assert_eq!(v.tail_window_turns, 12);
    assert!(v.collapse_enabled);
    assert_eq!(v.collapse_threshold_pct, 90);
    // 持久化
    let reopened = SettingsStore::open(&root).unwrap();
    assert_eq!(reopened.view().memory.tail_strategy, "window");
    assert_eq!(reopened.view().memory.tail_window_turns, 12);

    // 校验：策略白名单 / 窗口区间 / 阈值区间
    let err = store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                tail_strategy: Some("bogus".into()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
    assert!(err.contains("尾巴策略"), "{err}");
    let err = store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                tail_window_turns: Some(0),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
    assert!(err.contains("固定窗口"), "{err}");
    let err = store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                collapse_threshold_pct: Some(30),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
    assert!(err.contains("自动收起阈值"), "{err}");
    // 出界不污染内存态
    assert_eq!(store.view().memory.tail_window_turns, 12);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn retired_keys_are_phantom() {
    let root = temp_root("m5keys");
    SettingsStore::open(&root).unwrap(); // 建目录与缺省文件，再覆盖夹具
                                         // 幻影键（v2.136 退役三键 + v2.137 defaultOrganizer）：存量
                                         // settings.json 带着旧键照常解析，投影不出现、非退役键照常生效
    std::fs::write(
        root.join(super::FILE_NAME),
        r#"{"memory":{"autoExtract":false,"handoffUpdate":false,"experiences":true,"defaultOrganizer":"legacy","topK":7}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().memory;
    assert_eq!(v.top_k, 7, "非退役键照常生效");
    let json = serde_json::to_value(&v)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    for key in [
        "autoExtract",
        "handoffUpdate",
        "experiences",
        "defaultOrganizer",
    ] {
        assert!(!json.contains_key(key), "退役键 {key} 不进投影");
    }
    // 本批新增的两个键必须在投影里（管理页读的是 view 面，缺一个就是界面空白）
    for key in ["factCards", "factCardMax"] {
        assert!(json.contains_key(key), "新键 {key} 必须进投影");
    }
    std::fs::remove_dir_all(&root).ok();
}
