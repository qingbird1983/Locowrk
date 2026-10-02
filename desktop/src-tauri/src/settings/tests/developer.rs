//! v2.49 开发者选项区测试：缺省全关（隐藏 + 危险开关）/ 更新往返 /
//! 重启持久（解锁态跨重启）/ 普通字段不受影响。

use super::*;

fn dev_update(
    store: &SettingsStore,
    unlocked: Option<bool>,
    del: Option<bool>,
    sw: Option<bool>,
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
        }),
    )
}

#[test]
fn developer_defaults_off_roundtrip_and_persist() {
    let root = temp_root("developer");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省全关：分区隐藏、危险开关关闭（旧 settings.json 无此区零迁移）
    let v = store.view().developer;
    assert!(!v.unlocked);
    assert!(!v.allow_delete_running_session);
    assert!(!v.allow_switch_running_session);
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
