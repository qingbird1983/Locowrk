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
                auto_extract: Some(true),
                organize_pacing: None,
                organize_chunk_tokens: None,
                recorder_model: None,
                recorder_provider: None,
            }),
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
            )
            .is_err());
    }
    assert_eq!(store.view().memory.top_k, 8);
    std::fs::remove_dir_all(&root).ok();
}

// ---- M2 记忆树整理三键（树 v2 §4.3/§5.2/§15）：节奏 / 增量块 / 记录员 ----

#[test]
fn organize_keys_roundtrip_validation_and_bridge() {
    let root = temp_root("memory-organize");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：batch / 50K / 记录员未配（同主模型）
    let v = store.view().memory;
    assert_eq!(v.organize_pacing, "batch");
    assert_eq!(v.organize_chunk_tokens, 50_000);
    assert!(v.recorder_model.is_none());
    assert!(v.recorder_provider.is_none());
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
                auto_extract: None,
                organize_pacing: Some("incremental".into()),
                organize_chunk_tokens: Some(80_000),
                recorder_model: Some("deepseek-flash".into()),
                recorder_provider: Some("p1".into()),
            }),
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
    assert_eq!(v.recorder_model.as_deref(), Some("deepseek-flash"));
    assert_eq!(v.recorder_provider.as_deref(), Some("p1"));
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
        )
        .unwrap_err();
    assert!(err.contains("增量块"), "{err}");
    // 记录员模型传空串 = 清除（回落同主模型）
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                recorder_model: Some(String::new()),
                recorder_provider: Some(String::new()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(store.view().memory.recorder_model.is_none());
    std::fs::remove_dir_all(&root).ok();
}
