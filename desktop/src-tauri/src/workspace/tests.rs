//! workspace.rs 单测：上下文解析器走旁车真源（workspace + 围栏根）、
//! workspace 视图投影 + scope_key 归一。

use super::{scope_key, session_context_resolver, workspace_view};

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

    let data_root = dir.join("data");
    let resolver = session_context_resolver(dir.clone(), data_root.clone());
    let ctx1 = resolver("session-1");
    assert_eq!(ctx1.workspace.as_deref(), Some("F:/ws/a"));
    // ㉘·B 围栏跟会话：归属会话 = [数据根, 项目工作区]，且已规范化
    assert_eq!(ctx1.fs_roots.len(), 2);
    assert!(ctx1.fs_roots[1].replace('/', "\\").ends_with("ws\\a"));

    // 未归属：workspace=None + fs_roots 空（插件回落 env 全局语义）
    let ctx2 = resolver("session-2");
    assert_eq!(ctx2.workspace, None);
    assert!(ctx2.fs_roots.is_empty());
    // 非法 id / 缺旁车 fail-closed
    assert_eq!(resolver("../escape").workspace, None);
    assert_eq!(resolver("ghost").workspace, None);
}

#[test]
fn workspace_view_projects_workspace_only() {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-ws-view-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    session_store::write_workspace(&dir, "session-1", Some("F:/ws/a")).unwrap();
    let view = workspace_view(session_context_resolver(dir.clone(), dir.join("data")));
    assert_eq!(view("session-1").as_deref(), Some("F:/ws/a"));
    assert_eq!(view("ghost"), None);
}

#[test]
fn scope_key_normalizes_to_unfiled() {
    assert_eq!(scope_key(None), "");
    assert_eq!(scope_key(Some("")), "");
    assert_eq!(scope_key(Some("F:/ws/a")), "F:/ws/a");
}
