//! 上下文管理区（⑨）与模型窗口（S2 迁 llm 区）设置测试：缺省值 /
//! 旧字段迁移幂等 / 阈值换算与建议值 / 窗口校验。

use super::*;

// ---- 上下文管理区（⑨）与模型窗口（S2 迁 llm 区） ----

#[test]
fn context_defaults_and_legacy_json_compat() {
    let root = temp_root("context-defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().context;
    // 缺省：阈值 30% / 分层保留；窗口未知 → FALLBACK 保守保护
    assert_eq!(v.threshold_pct, 30);
    assert_eq!(v.strategy, kernel_agent::CompactStrategy::Layered);
    assert_eq!(v.threshold_tokens, 9_600, "默认 = FALLBACK_WINDOW × 30%");
    assert_eq!(v.suggested_pct, None, "窗口未知不给建议值");
    // 旧 settings.json 无 context 字段 → 字段级缺省，不解析失败
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.threshold_pct, 30);
    std::fs::remove_dir_all(&root).ok();
}

/// S2 迁移：旧 `context.windowTokens` 一次搬到 llm 区（信息归属——窗口是
/// 模型属性）；阈值换算与建议值随之改用新来源。
#[test]
fn llm_window_migrates_from_legacy_context_section() {
    let root = temp_root("window-migrate");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"locale":"zh-CN","context":{"windowTokens":262144,"thresholdPct":40}}"#,
    )
    .unwrap();
    let store = SettingsStore::open(&root).unwrap();
    assert_eq!(
        store.view().llm.window_tokens,
        Some(262_144),
        "旧 context 窗口迁移到 llm 区"
    );
    let v = store.view().context;
    assert_eq!(v.threshold_tokens, 262_144 * 40 / 100, "阈值按新来源换算");
    assert!(v.suggested_pct.is_some(), "窗口已知后给出建议值");
    // 迁移幂等：源字段已被置 None，再开一次不变
    let again = SettingsStore::open(&root).unwrap();
    assert_eq!(again.view().llm.window_tokens, Some(262_144));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn llm_window_update_validation_and_threshold_math() {
    let root = temp_root("window-update");
    let store = SettingsStore::open(&root).unwrap();
    store
        .update(
            None,
            None,
            Some(LlmUpdate {
                window_tokens: Some(Some(128_000)),
                ..Default::default()
            }),
            None,
            Some(ContextUpdate {
                threshold_pct: Some(40),
                strategy: Some(kernel_agent::CompactStrategy::SlidingWindow),
            }),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(store.view().llm.window_tokens, Some(128_000));
    assert_eq!(store.view().context.threshold_tokens, 51_200, "128k × 40%");
    // compact_config 桥接（ctx-compact 共享锁内容物）用同一把尺
    let cfg = store.compact_config();
    assert_eq!(cfg.threshold_tokens, 51_200);
    assert_eq!(cfg.strategy, kernel_agent::CompactStrategy::SlidingWindow);
    // windowTokens: null = 清除窗口 → 回落 32k 保守保护
    store
        .update(
            None,
            None,
            Some(LlmUpdate {
                window_tokens: Some(None),
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
    assert_eq!(store.view().llm.window_tokens, None);
    assert_eq!(store.view().context.threshold_tokens, 32_000 * 40 / 100);
    // 校验：窗口荒谬值拒绝且不污染内存态；阈值出界照旧拒绝
    assert!(store
        .update(
            None,
            None,
            Some(LlmUpdate {
                window_tokens: Some(Some(50)),
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
    assert!(store
        .update(
            None,
            None,
            Some(LlmUpdate {
                window_tokens: Some(Some(200_000_001)),
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
    assert!(store
        .update(
            None,
            None,
            None,
            None,
            Some(ContextUpdate {
                threshold_pct: Some(5),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
        )
        .is_err());
    assert_eq!(store.view().context.threshold_pct, 40);
    // 磁盘往返
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.threshold_pct, 40);
    assert_eq!(v.strategy, kernel_agent::CompactStrategy::SlidingWindow);
    std::fs::remove_dir_all(&root).ok();
}
