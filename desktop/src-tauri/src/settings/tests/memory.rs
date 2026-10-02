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
