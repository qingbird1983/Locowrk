//! 外观区设置测试（⑰）：缺省 dark / 旧文件兼容 / 更新往返 / 非法值拒绝。

use super::*;

#[test]
fn appearance_defaults_legacy_compat_and_update() {
    let root = temp_root("appearance");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省 dark——旧 settings.json 无 appearance 区时行为不变
    assert_eq!(store.view().appearance.theme, "dark");
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().appearance;
    assert_eq!(v.theme, "dark");
    // 更新 light + 磁盘往返
    store
        .update(
            None,
            Some(AppearanceUpdate {
                theme: Some("light".into()),
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().appearance;
    assert_eq!(v.theme, "light");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn rejects_unknown_theme() {
    let root = temp_root("appearance-invalid");
    let store = SettingsStore::open(&root).unwrap();
    assert!(store
        .update(
            None,
            Some(AppearanceUpdate {
                theme: Some("sepia".into()),
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .is_err());
    // 拒绝后内存态不被污染（仍为缺省 dark）
    assert_eq!(store.view().appearance.theme, "dark");
    std::fs::remove_dir_all(&root).ok();
}
