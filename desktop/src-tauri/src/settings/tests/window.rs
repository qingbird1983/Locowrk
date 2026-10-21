//! 上下文管理区（⑨）与模型窗口（S2 迁 llm 区）设置测试：缺省值 /
//! 旧字段迁移幂等 / 自动压缩阈值（specs/13：**占模型窗口的百分比**）校验 / 幻影键 /
//! session_window 四层窗口链 / session_capacity（S2 刀①：窗口与输出上限一次解析）。

use super::*;

// ---- 上下文管理区（⑨）与模型窗口（S2 迁 llm 区） ----

#[test]
fn context_defaults_and_legacy_json_compat() {
    let root = temp_root("context-defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().context;
    // 缺省：自动压缩阈值 50%（占模型窗口）+ 进度条开
    assert_eq!(v.auto_compact_pct, 50);
    assert!(v.usage_bar);
    // 旧 settings.json 无 context 字段 → 字段级缺省，不解析失败
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.auto_compact_pct, 50);
    // v2.137 前旧键（百分比/策略/稳定期）= 幻影键：读得进、新键走缺省
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"context":{"thresholdPct":40,"strategy":"sliding_window","stabilityRounds":7}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.auto_compact_pct, 50, "退役键幻影化：阈值走缺省");
    assert_eq!(
        store.auto_compact_pct(),
        50,
        "触发面同源（读的是同一格百分比）"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// S2 迁移：旧 `context.windowTokens` 一次搬到 llm 区（信息归属——窗口是
/// 模型属性）；specs/13 起阈值是**占窗口的百分比**，不随旧百分比换算。
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
        store.view().context.auto_compact_pct,
        50,
        "阈值是独立旋钮（占窗口百分比），不随旧百分比换算"
    );
    // 迁移幂等：源字段已被置 None，再开一次不变
    let again = SettingsStore::open(&root).unwrap();
    assert_eq!(again.view().llm.window_tokens, Some(262_144));
    std::fs::remove_dir_all(&root).ok();
}

/// specs/13：自动压缩阈值 = **占模型窗口的百分比**（1–100 的整数，缺省 50）。
///
/// 校验规矩与上一档一样：越界**拒绝**而不是钳到边界（设置面写的是用户意图，
/// 悄悄改掉等于骗人），且拒绝不得污染内存态。
#[test]
fn auto_compact_pct_update_validation_and_roundtrip() {
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
                auto_compact_pct: Some(85),
                usage_bar: None,
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
    assert_eq!(store.view().context.auto_compact_pct, 85);
    assert_eq!(store.auto_compact_pct(), 85);
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.auto_compact_pct, 85);
    // 校验：档外值拒绝（0 / 30 / 100 / 101 —— 范围里的 30 与 100 同样拒绝，
    // 因为后端是**校验权威**：只收三档。界面上非档值只可能来自手改过的
    // settings.json，档外值在界面上"就近显示但不改存量"，而存档时会被拒收。
    for bad in [0u64, 30, 100, 101, 1_000] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                None,
                Some(ContextUpdate {
                    auto_compact_pct: Some(bad),
                    usage_bar: None,
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
    assert_eq!(store.view().context.auto_compact_pct, 85);
    // 另一档也合法（25 / 50 / 85 三档都收）
    for ok in [25u64, 50] {
        store
            .update(
                None,
                None,
                None,
                None,
                None,
                Some(ContextUpdate {
                    auto_compact_pct: Some(ok),
                    usage_bar: None,
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
                auto_compact_pct: None,
                usage_bar: Some(false),
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
    let catalog_absent = crate::testutil::unique_dir("settings-tests-catalog-absent");
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

/// S2 刀① 新增：窗口与输出上限**同一次解析**（`session_capacity`）。
///
/// 为什么这条要单独立着而不是并入上面那条：上面那条判的是"窗口四层链的值"，
/// 本条判的是"两个读数不许来自两条链"——错配的形状是"窗口按覆盖档取、
/// 输出上限按活动档取"，那种数在界面上看不出来，只在压缩线上表现为
/// "该封顶时没封顶"。目录命中那一档不在这里断言：本夹具造不出目录内容
/// （`CatalogStore::new` 读空文件 = 空快照），那一档的证据在
/// `llm/catalog/tests.rs`（lookup 带出 `max_output = Some(393_216)`）。
#[test]
fn session_capacity_resolves_window_and_output_in_one_pass() {
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
            // 用户钉死的输出上限（settings 只存钉死值，与批 A 的口径一致）
            max_output: Some(8_192),
            max_output_source: Some("user".into()),
            ..Default::default()
        }],
        ..Default::default()
    });

    // 活动档模型没有行 ⇒ 窗口回落档位，输出上限两档都无 = None（不许从档位瞎猜）
    assert_eq!(
        crate::settings::session_capacity(&cfg, None, None),
        (Some(65_536), None),
        "档位有窗口、没有输出上限这颗字段"
    );

    // 覆盖到有行的模型 ⇒ 两个读数都落在**同一行**上
    let ov = ("p1".to_string(), "deepseek-v4-pro".to_string());
    assert_eq!(
        crate::settings::session_capacity(&cfg, Some(&ov), None),
        (Some(131_072), Some(8_192)),
        "模型行钉死窗口与钉死输出上限同源"
    );

    // 薄封装等价性：窗口那一半与 `session_window` 逐次同值（刀① 不许改窗口口径）
    for probe in [None, Some(&ov)] {
        assert_eq!(
            session_window(&cfg, probe, None),
            crate::settings::session_capacity(&cfg, probe, None).0,
            "session_window 只是取第一半，没有第二套算术"
        );
    }

    // 全未知 ⇒ 两半都 None（压缩侧据此走裁定 A，见 `context_budget`）
    cfg.providers[0].window_tokens = None;
    cfg.providers[0].models[0].context_window = None;
    cfg.providers[0].models[0].max_output = None;
    assert_eq!(
        crate::settings::session_capacity(&cfg, Some(&ov), None),
        (None, None),
        "未配置就报未配置，不许发明一个窗口或输出上限"
    );
}
