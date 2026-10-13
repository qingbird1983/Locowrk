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
            organize_rebound_k: None,
            organize_rebound_m: None,
            memory_manager_timeout_secs: None,
            memory_manager_max_retries: None,
            rule_chunk_tokens: None,
            organize_max_rerun: None,
            organize_max_release: None,
            organize_release_watermark_pct: None,
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

// ---- M2 记忆树整理（树 v2 §15 开发者层级）：熔断 k/m / 超时 / 重试 / 规则切分块 ----

#[test]
fn organize_developer_keys_roundtrip_and_validation() {
    let root = temp_root("developer-organize");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：3/3 / 60s / 3 次 / 20K
    let v = store.view().developer;
    assert_eq!(v.organize_rebound_k, 3);
    assert_eq!(v.organize_rebound_m, 3);
    // Q6（CB §7.2）把「超时 × 重试」**同调**：缺省 60→30、3→2 ⇒ 在途最坏窗口
    // 60×(1+3)=240s 收到 30×(1+2)=90s。这两行断言的改动是**正当改写既有判据**
    // （本批的交付物之一就是改这两个缺省），不是回归；同调的不变量由
    // `defaults_keep_the_in_flight_window_at_90_secs` 钉，防止下一批只改一个数。
    assert_eq!(v.memory_manager_timeout_secs, 30);
    assert_eq!(v.memory_manager_max_retries, 2);
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
                // Q6 新字段走同一条"写入—重开—复读"通路（缺省 3 之外给一个非缺省值，
                // 才证明它真的被应用而不是每次都退回 Default）
                organize_max_rerun: Some(5),
                // Q8b 同一条通路：给两个非缺省值（缺省 2 / 90），证明它们进的是
                // 同一次持久化而不是各自另开一条读写路径
                organize_max_release: Some(4),
                organize_release_watermark_pct: Some(75),
            }),
            None,
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
    assert_eq!(v.organize_max_rerun, 5);
    assert_eq!(v.organize_max_release, 4, "Q8b 泄压上限写入—重开—复读");
    assert_eq!(
        v.organize_release_watermark_pct, 75,
        "Q8b 泄压水位写入—重开—复读"
    );
    // 校验：k/m 1–10、超时 10–600、重试 0–10、规则切分块 5K–50K、补跑上限 0–10、
    // 泄压上限 0–5、泄压水位 50–99
    for (field, val, keyword) in [
        ("organize_rebound_k", 11, "反弹熔断 k"),
        ("organize_rebound_m", 0, "反弹熔断 m"),
        ("memory_manager_timeout_secs", 5, "记忆管理器超时"),
        ("memory_manager_max_retries", 11, "记忆管理器重试"),
        ("rule_chunk_tokens", 1_000, "规则切分块"),
        ("organize_max_rerun", 11, "连续补跑上限"),
        ("organize_max_release", 6, "轮内泄压上限"),
        ("organize_release_watermark_pct", 100, "泄压水位"),
    ] {
        let u = DeveloperUpdate {
            organize_rebound_k: (field == "organize_rebound_k").then_some(val),
            organize_rebound_m: (field == "organize_rebound_m").then_some(val),
            memory_manager_timeout_secs: (field == "memory_manager_timeout_secs")
                .then_some(val as u64),
            memory_manager_max_retries: (field == "memory_manager_max_retries").then_some(val),
            rule_chunk_tokens: (field == "rule_chunk_tokens").then_some(val as u64),
            organize_max_rerun: (field == "organize_max_rerun").then_some(val),
            organize_max_release: (field == "organize_max_release").then_some(val),
            organize_release_watermark_pct: (field == "organize_release_watermark_pct")
                .then_some(val as u8),
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
                Some(u),
                None,
                None,
            )
            .unwrap_err();
        assert!(err.contains(keyword), "{field}: {err}");
    }
    std::fs::remove_dir_all(&root).ok();
}

/// 在途窗口的**同调不变量**（队列 Q6 · CB §7.2 + §11 点名的坑）：缺省组合必须给出
/// 30s × (1+2) = 90s，且超时与重试**两个数一起改**才算"缩小盲区"。
///
/// 为什么必须有这条：稿 §11 写的坑是「60s 超时改 30s 但重试仍 3 次，盲区没变小」——
/// 那两个数各自看都像改过了，只有乘积能证伪。判据打在缺省常量上而不是打在用户可改的
/// 内存态上（用户设过的值不该被本批的缺省改动判红），变异 M6 打的就是"只回退一个数"。
#[test]
fn defaults_keep_the_in_flight_window_at_90_secs() {
    use crate::settings::developer as dev;
    let worst = dev::MANAGER_TIMEOUT_SECS_DEFAULT * (1 + dev::MANAGER_MAX_RETRIES_DEFAULT as u64);
    assert_eq!(
        worst, 90,
        "在途最坏窗口必须 = 30×(1+2)；只调一个数会停在 60 或 120"
    );
    assert_eq!(dev::MANAGER_TIMEOUT_SECS_DEFAULT, 30);
    assert_eq!(dev::MANAGER_MAX_RETRIES_DEFAULT, 2);
    // 补跑上限的缺省与非零语义（0 是合法值 = 关掉补跑，按可配置原则②）
    assert_eq!(dev::ORGANIZE_MAX_RERUN_DEFAULT, 3);
    assert!(dev::ORGANIZE_MAX_RERUN_RANGE.contains(&0));
    assert!(dev::ORGANIZE_MAX_RERUN_RANGE.contains(&3));
    assert!(dev::check_max_rerun(0).is_ok());
    assert!(dev::check_max_rerun(11).is_err());
}

/// 轮内泄压这一对的**缺省与开关语义**（队列 Q8b · 可配置原则①②）。
///
/// 为什么单独一条而不是并进上面那条：上面钉的是"在途窗口的乘积不变量"，
/// 这条钉的是"新增的一层限制自带关掉的路"——`organize_max_release` 的 0 必须
/// 落在合法区间内且文案点名 0 的语义，否则"限制没有开关"就写死在缺省里了。
/// 水位的上界取 99 而不是 100 也是判据对象（100 = 等到装不下才切，那一刀已经晚了）。
#[test]
fn release_defaults_carry_their_own_off_switch() {
    use crate::settings::developer as dev;
    assert_eq!(dev::ORGANIZE_MAX_RELEASE_DEFAULT, 2);
    assert_eq!(dev::ORGANIZE_RELEASE_WATERMARK_PCT_DEFAULT, 90);
    // 0 = 关掉轮内泄压（合法值，不是出界）
    assert!(dev::ORGANIZE_MAX_RELEASE_RANGE.contains(&0));
    assert!(dev::check_max_release(0).is_ok());
    assert!(dev::check_max_release(5).is_ok());
    assert!(dev::check_max_release(6).is_err());
    // 出界文案必须把"0 = 关掉"这层语义说出来（只报数字用户不知道该填什么）
    let err = dev::check_max_release(6).unwrap_err();
    assert!(err.contains("0 = 关掉泄压"), "{err}");
    // 水位区间的上界是 99：100 不许合法
    assert!(dev::ORGANIZE_RELEASE_WATERMARK_PCT_RANGE.contains(&50));
    assert!(dev::ORGANIZE_RELEASE_WATERMARK_PCT_RANGE.contains(&99));
    assert!(dev::check_release_watermark(100).is_err());
    assert!(dev::check_release_watermark(49).is_err());
    assert!(dev::check_release_watermark(90).is_ok());
}
