//! OrganizerModeStore 测试：缺省回落 / 显式缓存 / 写穿 / 清除跟随缺省。

use std::path::PathBuf;

use super::{OrganizerMode, OrganizerModeStore};
use session_store::{append_event, session_file};

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("locowrk-orgmode-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn seed(dir: &std::path::Path, id: &str) {
    append_event(
        &session_file(dir, id).unwrap(),
        &session_store::SessionEvent::UserMessage {
            content: "hi".into(),
        },
    )
    .unwrap();
}

#[test]
fn mode_store_default_fallback_and_sidecar_roundtrip() {
    let dir = tmp_dir("basic");
    seed(&dir, "s1");
    let store = OrganizerModeStore::new(dir.clone(), OrganizerMode::Legacy);
    // 无旁车 = 缺省档；None 会话 id 同
    assert_eq!(store.mode_of(Some("s1")), OrganizerMode::Legacy);
    assert_eq!(store.mode_of(None), OrganizerMode::Legacy);
    // 显式设置 = 写穿 + 缓存命中
    store.set_session_mode("s1", OrganizerMode::Tree).unwrap();
    assert_eq!(store.mode_of(Some("s1")), OrganizerMode::Tree);
    // 新实例（冷启动）从旁车恢复显式档
    let store2 = OrganizerModeStore::new(dir.clone(), OrganizerMode::Legacy);
    assert_eq!(store2.mode_of(Some("s1")), OrganizerMode::Tree);
    // 清除 = 跟随缺省；缺省热换后未设置会话跟随新缺省
    store2.clear_session_mode("s1").unwrap();
    assert_eq!(store2.mode_of(Some("s1")), OrganizerMode::Legacy);
    store2.set_default(OrganizerMode::Tree);
    assert_eq!(store2.mode_of(Some("s1")), OrganizerMode::Tree);
    assert_eq!(store2.mode_of(None), OrganizerMode::Tree);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn mode_store_unknown_sidecar_value_falls_back_to_default() {
    let dir = tmp_dir("unknown");
    seed(&dir, "s1");
    session_store::write_organizer(&dir, "s1", Some("bogus")).unwrap();
    let store = OrganizerModeStore::new(dir.clone(), OrganizerMode::Tree);
    assert_eq!(
        store.mode_of(Some("s1")),
        OrganizerMode::Tree,
        "手改未知值不缓存，跟随缺省档"
    );
    std::fs::remove_dir_all(&dir).ok();
}
