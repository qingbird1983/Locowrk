//! 上下文管理区（⑨）与模型窗口（S2 迁 llm 区）设置测试：缺省值 /
//! 旧字段迁移幂等 / 整理阈值（v2.137 绝对 token）校验 / 幻影键 /
//! session_window 四层窗口链。

use super::*;

// ---- 上下文管理区（⑨）与模型窗口（S2 迁 llm 区） ----

#[test]
fn context_defaults_and_legacy_json_compat() {
    let root = temp_root("context-defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().context;
    // 缺省：整理阈值 100K（绝对 token）+ 进度条开
    assert_eq!(v.threshold_tokens, 100_000);
    assert!(v.usage_bar);
    // 旧 settings.json 无 context 字段 → 字段级缺省，不解析失败
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.threshold_tokens, 100_000);
    // v2.137 前旧键（百分比/策略/稳定期）= 幻影键：读得进、新键走缺省
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"context":{"thresholdPct":40,"strategy":"sliding_window","stabilityRounds":7}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.threshold_tokens, 100_000, "退役键幻影化：阈值走缺省");
    assert_eq!(
        store.organize_threshold_tokens(),
        100_000,
        "触发面同源（不再折算窗口百分比）"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// S2 迁移：旧 `context.windowTokens` 一次搬到 llm 区（信息归属——窗口是
/// 模型属性）；v2.137 起阈值不再吃窗口，迁移只搬窗口。
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
    assert_eq!(
        store.view().context.threshold_tokens,
        100_000,
        "阈值是独立旋钮，不随旧百分比换算"
    );
    // 迁移幂等：源字段已被置 None，再开一次不变
    let again = SettingsStore::open(&root).unwrap();
    assert_eq!(again.view().llm.window_tokens, Some(262_144));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn organize_threshold_update_validation_and_roundtrip() {
    let root = temp_root("threshold-update");
    let store = SettingsStore::open(&root).unwrap();
    // 合法档位 + 磁盘往返（重开读回 = 永久保存的硬证明）
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            Some(ContextUpdate {
                threshold_tokens: Some(300_000),
                usage_bar: None,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(store.view().context.threshold_tokens, 300_000);
    assert_eq!(store.organize_threshold_tokens(), 300_000);
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.threshold_tokens, 300_000);
    // 校验：出界拒绝（下限下方 / 上限上方）且不污染内存态
    for bad in [999u64, 99_999, 800_001, 10_000_000] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                None,
                Some(ContextUpdate {
                    threshold_tokens: Some(bad),
                    usage_bar: None,
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .is_err());
    }
    assert_eq!(store.view().context.threshold_tokens, 300_000);
    // 边界值合法
    for ok in [100_000u64, 800_000] {
        store
            .update(
                None,
                None,
                None,
                None,
                None,
                Some(ContextUpdate {
                    threshold_tokens: Some(ok),
                    usage_bar: None,
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
    }
    std::fs::remove_dir_all(&root).ok();
}

/// v4.18（UI shell §3.4）：阈值用量进度条显隐——缺省开 / 更新关 /
/// 磁盘往返（旧文件缺字段走 serde 容器级 default = 开，零迁移）。
#[test]
fn context_usage_bar_default_and_roundtrip() {
    let root = temp_root("context-usage-bar");
    let store = SettingsStore::open(&root).unwrap();
    assert!(store.view().context.usage_bar, "缺省开");

    store
        .update(
            None,
            None,
            None,
            None,
            None,
            Some(ContextUpdate {
                threshold_tokens: None,
                usage_bar: Some(false),
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(!store.view().context.usage_bar, "关闭生效");

    drop(store);
    assert!(
        !SettingsStore::open(&root).unwrap().view().context.usage_bar,
        "重开软件后逐字段读回"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// v2.137 会话生效窗口四层链：模型行 context_window（钉死面）→ 目录 →
/// 档位 window_tokens；会话模型覆盖改判档。目录缺席 = 空快照（该层跳过）。
#[test]
fn session_window_resolution_chain() {
    let catalog_absent = std::env::temp_dir().join("settings-tests-catalog-absent");
    let mut cfg = LlmConfig {
        active: "p1".into(),
        ..Default::default()
    };
    cfg.providers.push(ProviderProfile {
        id: "p1".into(),
        label: "deepseek".into(),
        kind: "deepseek".into(),
        base_url: Some("https://api.deepseek.com/v1".into()),
        model: "deepseek-flash".into(),
        window_tokens: Some(65_536),
        enabled: true,
        models: vec![ProviderModel {
            id: "deepseek-v4-pro".into(),
            context_window: Some(131_072),
            ..Default::default()
        }],
        ..Default::default()
    });

    // 活动档模型无行 → 回落档位 window_tokens
    assert_eq!(
        session_window(&cfg, None, None),
        Some(65_536),
        "档位 window_tokens 兜底"
    );

    // 会话覆盖到有行模型 → 模型行钉死窗口优先
    let ov = ("p1".to_string(), "deepseek-v4-pro".to_string());
    assert_eq!(
        session_window(&cfg, Some(&ov), None),
        Some(131_072),
        "模型行 context_window 优先"
    );

    // 覆盖到未知档 → 回落活动档档位窗口（不因覆盖档缺失而丢失口径）
    let bad_ov = ("pX".to_string(), "m".to_string());
    assert_eq!(session_window(&cfg, Some(&bad_ov), None), Some(65_536));

    // 目录在位（空快照 = 无命中，跳过该层）；档位窗口未设 → 全未知
    cfg.providers[0].window_tokens = None;
    assert_eq!(
        session_window(
            &cfg,
            None,
            Some(&crate::llm::catalog::CatalogStore::new(catalog_absent))
        ),
        None,
        "全未知 = None（前端按未设置呈现）"
    );
}
