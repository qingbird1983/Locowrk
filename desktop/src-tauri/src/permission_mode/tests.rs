//! 权限模式存储测试（㉘·B）：缓存命中、旁车持久与回落、缺省热换。

use super::{PermissionMode, PermissionModeStore};
use std::fs;

fn tmp_sessions(tag: &str) -> std::path::PathBuf {
    let dir = crate::testutil::work_dir(&format!("lw-perm-mode-{}", tag));
    dir
}

/// 缺省档：无会话 = 缺省；未知会话 = 缺省；未知会话不写缓存副作用也无所谓
/// （幂等回落）。
#[test]
fn falls_back_to_default_for_unknown_session() {
    let dir = tmp_sessions("fallback");
    let store = PermissionModeStore::new(dir.clone(), PermissionMode::Ask);
    assert_eq!(store.mode_of(None), PermissionMode::Ask);
    assert_eq!(store.mode_of(Some("ghost-session")), PermissionMode::Ask);
    fs::remove_dir_all(dir).ok();
}

/// 设置会话档写穿旁车 + 缓存命中；重新构造存储（模拟重启）后仍从旁车
/// 恢复同一档——持久化的硬证明。
#[test]
fn set_then_reopen_reads_persisted_mode() {
    let dir = tmp_sessions("persist");
    let store = PermissionModeStore::new(dir.clone(), PermissionMode::Ask);
    store
        .set_session_mode("sess-a", PermissionMode::Auto)
        .expect("write sidecar");
    assert_eq!(store.mode_of(Some("sess-a")), PermissionMode::Auto);

    // 重启等价：新存储实例从旁车读回
    let reopened = PermissionModeStore::new(dir.clone(), PermissionMode::Ask);
    assert_eq!(reopened.mode_of(Some("sess-a")), PermissionMode::Auto);
    fs::remove_dir_all(dir).ok();
}

/// 旁车里的未知模式串 / 缺字段回落设置缺省（且**不缓存**——缺省热换后
/// 未显式设置的会话跟随新缺省）；显式设置过的会话不受缺省热换影响。
#[test]
fn unknown_sidecar_value_falls_back_and_default_hot_swaps() {
    let dir = tmp_sessions("hotswap");
    let meta = r#"{"permissionMode": "bogus"}"#;
    fs::write(dir.join("sess-b.meta.json"), meta).expect("seed sidecar");
    let store = PermissionModeStore::new(dir.clone(), PermissionMode::Ask);
    assert_eq!(store.mode_of(Some("sess-b")), PermissionMode::Ask);

    // 缺省热换：未显式设置的会话跟随新缺省（未被缓存冻结）
    store.set_default(PermissionMode::AcceptEdits);
    assert_eq!(store.mode_of(Some("sess-b")), PermissionMode::AcceptEdits);

    // 显式设置的会话在热换后保持自己的档
    store
        .set_session_mode("sess-c", PermissionMode::Plan)
        .expect("write sidecar");
    store.set_default(PermissionMode::Auto);
    assert_eq!(store.mode_of(Some("sess-c")), PermissionMode::Plan);
    fs::remove_dir_all(dir).ok();
}
