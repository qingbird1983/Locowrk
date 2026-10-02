//! 外观区设置测试（⑰；UI 设计 v3.3 起缺省 light）：缺省 light / 旧文件
//! 兼容（无 appearance 区也随缺省变亮色，§8.3-1 裁定）/ 更新往返 / 非法值拒绝。

use super::*;

#[test]
fn appearance_defaults_legacy_compat_and_update() {
    let root = temp_root("appearance");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省 light——用户 2026-10-01 裁定；旧 settings.json 无 appearance 区
    // 时也随缺省变亮色（属预期），显式存过 dark 的文件不受影响
    assert_eq!(store.view().appearance.theme, "light");
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().appearance;
    assert_eq!(v.theme, "light");
    std::fs::write(
        root.join("settings.json"),
        r#"{"locale": "zh-CN", "appearance": {"theme": "dark"}}"#,
    )
    .unwrap();
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
            None,
            None,
        )
        .is_err());
    // 拒绝后内存态不被污染（仍为缺省 light）
    assert_eq!(store.view().appearance.theme, "light");
    std::fs::remove_dir_all(&root).ok();
}
