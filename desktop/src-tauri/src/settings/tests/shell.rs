//! S1 shell 治理区设置测试：缺省关 / 旧文件兼容 / 更新往返 / env 桥。

use super::*;

#[test]
fn shell_defaults_legacy_compat_and_update() {
    let root = temp_root("shell");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：解锁关——高危命令默认拒绝（清单 §2.7 约束最小化）
    assert!(!store.view().shell.unlock_risky);
    assert!(!store.shell_unlock_risky());
    // 旧 settings.json 无 shell 区 → 字段级缺省
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().shell;
    assert!(!v.unlock_risky);
    // 更新 + 磁盘往返 + store accessor（state.rs 写 env 的数据源）
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
            Some(ShellUpdate {
                unlock_risky: Some(true),
            }),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(store.view().shell.unlock_risky);
    assert!(SettingsStore::open(&root).unwrap().shell_unlock_risky());
}
