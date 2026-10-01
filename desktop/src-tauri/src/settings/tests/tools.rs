//! ⑫ 工具暴露区设置测试：缺省/旧文件兼容/校验/生效配置桥接。
//! S2 起本区还承载宿主单次调用超时（callTimeoutSecs）。

use super::*;

#[test]
fn tools_defaults_legacy_compat_and_validation() {
    let root = temp_root("tools");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：开 / 20（exposure 文档 §5.3 Top-N）/ 超时 30s（S2）
    let v = store.view().tools;
    assert!(v.enabled);
    assert_eq!(v.max_per_request, 20);
    assert_eq!(v.call_timeout_secs, 30);
    assert_eq!(store.exposure_config().max_tools, 20);
    assert_eq!(store.call_timeout(), std::time::Duration::from_secs(30));
    // 旧 settings.json 无 tools 区 → 字段级缺省
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().tools;
    assert!(v.enabled);
    assert_eq!(v.max_per_request, 20);
    assert_eq!(v.call_timeout_secs, 30);
    // 更新 + 磁盘往返 + 生效配置桥接
    store
        .update(
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
            }),
            None,
            None,
        )
        .unwrap();
    assert!(!store.view().tools.enabled);
    assert_eq!(store.view().tools.max_per_request, 15);
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
            Some(ToolsUpdate {
                max_per_request: Some(30),
                ..Default::default()
            }),
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
                Some(ToolsUpdate {
                    max_per_request: Some(bad),
                    ..Default::default()
                }),
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
                Some(ToolsUpdate {
                    call_timeout_secs: Some(bad),
                    ..Default::default()
                }),
                None,
                None,
            )
            .is_err());
    }
    assert_eq!(store.view().tools.call_timeout_secs, 45);
    std::fs::remove_dir_all(&root).ok();
}
