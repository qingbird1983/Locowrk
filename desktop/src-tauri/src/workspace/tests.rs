//! workspace.rs 单测：解析器走旁车真源 + scope_key 归一。

use super::{scope_key, session_workspace_resolver};

#[test]
fn resolver_reads_sidecar_and_fails_closed() {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-ws-resolver-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    session_store::write_workspace(&dir, "session-1", Some("F:/ws/a")).unwrap();
    // workspace=None 且无标题 → 旁车整文件删除 → 解析为 None
    session_store::write_workspace(&dir, "session-2", None).unwrap();

    let resolver = session_workspace_resolver(dir.clone());
    assert_eq!(resolver("session-1").as_deref(), Some("F:/ws/a"));
    assert_eq!(resolver("session-2"), None);
    // 非法 id / 缺旁车 fail-closed
    assert_eq!(resolver("../escape"), None);
    assert_eq!(resolver("ghost"), None);
}

#[test]
fn scope_key_normalizes_to_unfiled() {
    assert_eq!(scope_key(None), "");
    assert_eq!(scope_key(Some("")), "");
    assert_eq!(scope_key(Some("F:/ws/a")), "F:/ws/a");
}
