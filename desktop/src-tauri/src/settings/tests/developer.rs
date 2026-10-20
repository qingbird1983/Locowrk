//! v2.49 开发者选项区测试：缺省全关（隐藏 + 危险开关）/ 更新往返 /
//! 重启持久（解锁态跨重启）/ 普通字段不受影响。

use super::*;

fn dev_update(
    store: &SettingsStore,
    unlocked: Option<bool>,
    del: Option<bool>,
    sw: Option<bool>,
) -> Result<SettingsView, String> {
    dev_update_full(store, unlocked, del, sw, None, None)
}

fn dev_update_full(
    store: &SettingsStore,
    unlocked: Option<bool>,
    del: Option<bool>,
    sw: Option<bool>,
    alert: Option<bool>,
    alert_percent: Option<u8>,
) -> Result<SettingsView, String> {
    store.update(
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
        Some(DeveloperUpdate {
            unlocked,
            allow_delete_running_session: del,
            allow_switch_running_session: sw,
            cache_diagnostics: None,
            cache_hit_alert: alert,
            cache_hit_alert_percent: alert_percent,
        }),
        None,
        None,
    )
}

#[test]
fn developer_defaults_off_roundtrip_and_persist() {
    let root = temp_root("developer");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省全关：分区隐藏、危险开关关闭（旧 settings.json 无此区零迁移）；
    // 告警关 + 阈值缺省 80（M0 观测）
    let v = store.view().developer;
    assert!(!v.unlocked);
    assert!(!v.allow_delete_running_session);
    assert!(!v.allow_switch_running_session);
    assert!(!v.cache_hit_alert);
    assert_eq!(
        v.cache_hit_alert_percent,
        super::super::developer::CACHE_HIT_ALERT_PERCENT_DEFAULT
    );
    // 旧文件兼容
    std::fs::write(root.join("settings.json"), r#"{"locale": "en"}"#).unwrap();
    assert!(
        !SettingsStore::open(&root)
            .unwrap()
            .view()
            .developer
            .unlocked
    );

    // 解锁 + 开一个危险开关；部分更新只动解锁态
    dev_update(&store, Some(true), Some(true), None).unwrap();
    let v = store.view().developer;
    assert!(v.unlocked);
    assert!(v.allow_delete_running_session);
    assert!(!v.allow_switch_running_session, "None = 保持不变");
    // 后端直读口（session_delete 忙锁守卫用）同一份值
    assert!(store.developer().allow_delete_running_session);

    // 锁回：unlocked=false 不影响已开的危险开关
    dev_update(&store, Some(false), None, None).unwrap();
    let v = store.view().developer;
    assert!(!v.unlocked);
    assert!(v.allow_delete_running_session);

    // 重启持久：drop 重开逐字段读回
    drop(store);
    let v = SettingsStore::open(&root).unwrap().view().developer;
    assert!(!v.unlocked);
    assert!(v.allow_delete_running_session);
    assert!(!v.allow_switch_running_session);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn developer_cache_alert_roundtrip_and_invalid_percent() {
    let root = temp_root("developer-alert");
    let store = SettingsStore::open(&root).unwrap();
    // 开告警 + 改阈值；部分更新只动阈值
    dev_update_full(&store, None, None, None, Some(true), Some(60)).unwrap();
    let v = store.view().developer;
    assert!(v.cache_hit_alert);
    assert_eq!(v.cache_hit_alert_percent, 60);
    dev_update_full(&store, None, None, None, None, Some(95)).unwrap();
    let v = store.view().developer;
    assert!(v.cache_hit_alert, "None = 保持不变");
    assert_eq!(v.cache_hit_alert_percent, 95);
    // 非法阈值拒绝且不污染内存态
    assert!(dev_update_full(&store, None, None, None, None, Some(0)).is_err());
    assert!(dev_update_full(&store, None, None, None, None, Some(101)).is_err());
    assert_eq!(store.view().developer.cache_hit_alert_percent, 95);
    // 重启持久
    drop(store);
    let v = SettingsStore::open(&root).unwrap().view().developer;
    assert!(v.cache_hit_alert);
    assert_eq!(v.cache_hit_alert_percent, 95);
    std::fs::remove_dir_all(&root).ok();
}
