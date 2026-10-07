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
        None,
        Some(DeveloperUpdate {
            unlocked,
            allow_delete_running_session: del,
            allow_switch_running_session: sw,
            cache_diagnostics: None,
            cache_hit_alert: alert,
            cache_hit_alert_percent: alert_percent,
            organize_rebound_k: None,
            organize_rebound_m: None,
            memory_manager_timeout_secs: None,
            memory_manager_max_retries: None,
            rule_chunk_tokens: None,
        }),
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

// ---- M2 记忆树整理（树 v2 §15 开发者层级）：熔断 k/m / 超时 / 重试 / 规则切分块 ----

#[test]
fn organize_developer_keys_roundtrip_and_validation() {
    let root = temp_root("developer-organize");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：3/3 / 60s / 3 次 / 20K
    let v = store.view().developer;
    assert_eq!(v.organize_rebound_k, 3);
    assert_eq!(v.organize_rebound_m, 3);
    assert_eq!(v.memory_manager_timeout_secs, 60);
    assert_eq!(v.memory_manager_max_retries, 3);
    assert_eq!(v.rule_chunk_tokens, 20_000);
    // 更新 + 磁盘往返
    store
        .update(
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
            None,
            Some(DeveloperUpdate {
                unlocked: None,
                allow_delete_running_session: None,
                allow_switch_running_session: None,
                cache_diagnostics: None,
                cache_hit_alert: None,
                cache_hit_alert_percent: None,
                organize_rebound_k: Some(5),
                organize_rebound_m: Some(2),
                memory_manager_timeout_secs: Some(120),
                memory_manager_max_retries: Some(1),
                rule_chunk_tokens: Some(30_000),
            }),
            None,
        )
        .unwrap();
    drop(store);
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().developer;
    assert_eq!(v.organize_rebound_k, 5);
    assert_eq!(v.organize_rebound_m, 2);
    assert_eq!(v.memory_manager_timeout_secs, 120);
    assert_eq!(v.memory_manager_max_retries, 1);
    assert_eq!(v.rule_chunk_tokens, 30_000);
    // 校验：k/m 1–10、超时 10–600、重试 0–10、规则切分块 5K–50K
    for (field, val, keyword) in [
        ("organize_rebound_k", 11, "反弹熔断 k"),
        ("organize_rebound_m", 0, "反弹熔断 m"),
        ("memory_manager_timeout_secs", 5, "记忆管理器超时"),
        ("memory_manager_max_retries", 11, "记忆管理器重试"),
        ("rule_chunk_tokens", 1_000, "规则切分块"),
    ] {
        let u = DeveloperUpdate {
            organize_rebound_k: (field == "organize_rebound_k").then_some(val),
            organize_rebound_m: (field == "organize_rebound_m").then_some(val),
            memory_manager_timeout_secs: (field == "memory_manager_timeout_secs")
                .then_some(val as u64),
            memory_manager_max_retries: (field == "memory_manager_max_retries").then_some(val),
            rule_chunk_tokens: (field == "rule_chunk_tokens").then_some(val as u64),
            ..Default::default()
        };
        let err = store
            .update(
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
                None,
                Some(u),
                None,
            )
            .unwrap_err();
        assert!(err.contains(keyword), "{field}: {err}");
    }
    std::fs::remove_dir_all(&root).ok();
}
