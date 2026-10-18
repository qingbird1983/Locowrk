//! 记忆树与压缩那两组键（M2 整理三键 / M4 尾巴策略 + collapse）的设置测试。
//!
//! 为什么从 `memory.rs` 拆出来（S1 刀④b 那一批把它顶到 487 行）：这两组键属于**树模式会话**
//! （整理节奏 / 增量块 / 记忆管理器 / 尾巴策略 / collapse 阈值），判的是「区间与白名单」；
//! 留在原文件里，那个文件就同时管「注入区本体」与「树与压缩」两件事，三问自检第①问答「否」。
//! 用例本体逐字节搬移，一条断言都没动——搬文件不该改变行为，这是这类拆分的唯一硬判据。

use super::*;

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
                global_layer: None,
                project_layer: None,
                instruction_files: None,
                instruction_global_kib: None,
                instruction_project_kib: None,
                fact_cards: None,
                fact_card_max: None,
                learn_enabled: None,
                relevance_weight: None,
                strength_weight: None,
                freshness_weight: None,
                recency_weight: None,
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
