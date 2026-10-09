//! ⑫ 工具暴露区设置测试：缺省/旧文件兼容/校验/生效配置桥接。
//! S2 起本区还承载宿主单次调用超时（callTimeoutSecs）。

use super::*;

#[test]
fn tools_defaults_legacy_compat_and_validation() {
    let root = temp_root("tools");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：开 / 20（exposure 文档 §5.3 Top-N）/ 超时 30s（S2）/ 步数上限 8
    let v = store.view().tools;
    assert!(v.enabled);
    assert_eq!(v.max_per_request, 20);
    assert_eq!(v.call_timeout_secs, 30);
    assert_eq!(v.max_turn_steps, 8);
    assert!(v.fs_roots.is_empty(), "缺省无工作区目录（⑱：仅数据根）");
    assert_eq!(store.exposure_config().max_tools, 20);
    assert_eq!(store.call_timeout(), std::time::Duration::from_secs(30));
    // 旧 settings.json 无 tools 区 → 字段级缺省
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().tools;
    assert!(v.enabled);
    assert_eq!(v.max_per_request, 20);
    assert_eq!(v.call_timeout_secs, 30);
    assert_eq!(v.max_turn_steps, 8);
    // 更新 + 磁盘往返 + 生效配置桥接
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ToolsUpdate {
                enabled: Some(false),
                max_per_request: Some(15),
                call_timeout_secs: Some(45),
                max_turn_steps: Some(24),
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
    assert!(!store.view().tools.enabled);
    assert_eq!(store.view().tools.max_per_request, 15);
    assert_eq!(store.view().tools.max_turn_steps, 24);
    assert_eq!(store.view().tools.call_timeout_secs, 45);
    assert_eq!(
        store.call_timeout(),
        std::time::Duration::from_secs(45),
        "Timeout::hot 共享锁的内容物来源"
    );
    let cfg = store.exposure_config();
    assert!(!cfg.enabled);
    assert_eq!(cfg.max_tools, 15);
    // 部分更新：只给 maxPerRequest，其余保持
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ToolsUpdate {
                max_per_request: Some(30),
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
    assert!(!store.view().tools.enabled);
    assert_eq!(store.view().tools.max_per_request, 30);
    assert_eq!(store.view().tools.call_timeout_secs, 45);
    // 校验：出界拒绝且不污染内存态
    for bad in [0, 201] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(ToolsUpdate {
                    max_per_request: Some(bad),
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
    assert_eq!(store.view().tools.max_per_request, 30);
    for bad in [4, 3601] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(ToolsUpdate {
                    call_timeout_secs: Some(bad),
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
    assert_eq!(store.view().tools.call_timeout_secs, 45);
    // ㉕·微调③：单轮步数上限出界拒绝且不污染内存态
    for bad in [0, 101] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(ToolsUpdate {
                    max_turn_steps: Some(bad),
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
    assert_eq!(store.view().tools.max_turn_steps, 24);
    // ⑱ fs_roots：存真实存在的目录（拿 temp root 自身），往返 + 去重去空
    let dir = root.display().to_string();
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ToolsUpdate {
                fs_roots: Some(vec![format!("  {dir}  "), String::new(), dir.clone()]),
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
    assert_eq!(
        store.view().tools.fs_roots,
        vec![dir.clone()],
        "trim + 去空 + 去重"
    );
    // ㉕·fix 重启持久化证据：drop 后重开同一数据根逐字段读回（用户裁定
    // 语义：步数上限改 20 → 重启/新会话仍按 20，永不自动复位到缺省）
    drop(store);
    let reopened = SettingsStore::open(&root).unwrap();
    let v = reopened.view().tools;
    assert_eq!(v.max_turn_steps, 24, "重开软件后步数上限不复位");
    assert_eq!(v.call_timeout_secs, 45);
    assert_eq!(v.max_per_request, 30);
    assert_eq!(v.fs_roots, vec![dir.clone()]);
    std::fs::remove_dir_all(&root).ok();
}

/// ⑱ fs_roots 归一化校验 + env 注入（`LOCOWRK_FS_ROOTS` 是进程全局，
/// 静态锁串行并在结束后还原，避免污染并行测试）。
#[test]
fn fs_roots_normalize_reject_and_env_injection() {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var("LOCOWRK_FS_ROOTS").ok();

    // 归一化：trim / 去空 / 去重保序（目录必须真实存在才能通过校验）
    let root = temp_root("fs-roots");
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("b")).unwrap();
    let a = root.join("a").display().to_string();
    let b = root.join("b").display().to_string();
    let got = super::super::tools::normalize_fs_roots(&[
        format!("  {a}  "),
        String::new(),
        a.clone(),
        b.clone(),
    ])
    .unwrap();
    assert_eq!(got, vec![a, b]);

    // 不存在的目录 → 拒绝（fail-closed，同 fsutil 使用时契约）
    let missing = std::env::temp_dir().join("locowrk-fs-roots-missing-dir");
    assert!(super::super::tools::normalize_fs_roots(&[missing.display().to_string()]).is_err());

    // env 注入：非空 = `;` 连接；空表 = 移除 env
    let store = SettingsStore::open(&root).unwrap();
    let dir = root.display().to_string();
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ToolsUpdate {
                fs_roots: Some(vec![dir.clone()]),
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
    crate::data_root::apply_fs_roots_env(&store);
    assert_eq!(std::env::var("LOCOWRK_FS_ROOTS").unwrap(), dir);
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ToolsUpdate {
                fs_roots: Some(Vec::new()),
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
    crate::data_root::apply_fs_roots_env(&store);
    assert!(std::env::var("LOCOWRK_FS_ROOTS").is_err(), "空表移除 env");

    match prev {
        Some(v) => std::env::set_var("LOCOWRK_FS_ROOTS", v),
        None => std::env::remove_var("LOCOWRK_FS_ROOTS"),
    }
    std::fs::remove_dir_all(&root).ok();
}

/// v2.49 可配置原则：审批守夜阈值可调（缺省 100；0 = 不限；出界拒绝且
/// 不污染内存态；重启持久）。
#[test]
fn approval_timeout_configurable() {
    let root = temp_root("approval-timeout");
    let store = SettingsStore::open(&root).unwrap();
    assert_eq!(store.view().tools.approval_timeout_secs, 100);
    assert_eq!(
        store.approval_timeout_secs(),
        100,
        "后端专用口（ApprovalHub 初值来源）"
    );

    // 合法三档：100 → 30 → 0（不限）
    for secs in [30, 0] {
        store
            .update(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(ToolsUpdate {
                    approval_timeout_secs: Some(secs),
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
        assert_eq!(store.view().tools.approval_timeout_secs, secs);
    }

    // 出界拒绝且不污染内存态（86401 > 上限）
    assert!(store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ToolsUpdate {
                approval_timeout_secs: Some(86_401),
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
    assert_eq!(store.view().tools.approval_timeout_secs, 0);

    // 重启持久
    drop(store);
    assert_eq!(
        SettingsStore::open(&root).unwrap().approval_timeout_secs(),
        0,
        "重开软件后不限档不复位"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// M1（KV v2 §4.2）：工具集会话钉死——缺省开 / 旧文件字段级缺省 /
/// 更新往返 / exposure_config 桥接。
#[test]
fn pin_session_roundtrip_and_legacy_compat() {
    let root = temp_root("pin-session");
    let store = SettingsStore::open(&root).unwrap();
    assert!(store.view().tools.pin_session, "缺省开（前缀纪律缺省收紧）");
    assert!(store.exposure_config().pin_session, "生效配置桥接");

    // 旧 settings.json 无 pinSession 字段 → 字段级缺省
    std::fs::write(root.join(FILE_NAME), r#"{"locale": "zh-CN"}"#).unwrap();
    let store = SettingsStore::open(&root).unwrap();
    assert!(store.view().tools.pin_session);

    // 关闭 + 磁盘往返
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ToolsUpdate {
                pin_session: Some(false),
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
    assert!(!store.view().tools.pin_session);
    assert!(!store.exposure_config().pin_session);
    drop(store);
    assert!(
        !SettingsStore::open(&root).unwrap().view().tools.pin_session,
        "重开软件后逐字段读回"
    );
    std::fs::remove_dir_all(&root).ok();
}
