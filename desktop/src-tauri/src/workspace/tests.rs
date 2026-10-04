//! workspace.rs 单测：上下文解析器走旁车真源（workspace + 围栏根）、
//! workspace 视图投影 + scope_key 归一。㉘·C：围栏 = 全局声明根（数据根
//! ∪ settings fsRoots），所有档位一致、不再按归属收窄。

use std::sync::Arc;

use super::{scope_key, session_context_resolver, workspace_view};

fn no_roots() -> Arc<dyn Fn() -> Vec<String> + Send + Sync> {
    Arc::new(Vec::new)
}

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
    let fs_roots = vec!["F:/ws/a".to_string(), "F:/ws/b".to_string()];
    let fs_roots2 = fs_roots.clone();
    let resolver = session_context_resolver(
        dir.clone(),
        data_root.clone(),
        Arc::new(move || fs_roots2.clone()),
    );
    let ctx1 = resolver("session-1");
    assert_eq!(ctx1.workspace.as_deref(), Some("F:/ws/a"));
    // ㉘·C 围栏让路：归属会话 = [数据根] ∪ fsRoots（声明根全集，不按归属
    // 收窄——跨项目访问在声明根内畅通），且已规范化
    assert_eq!(ctx1.fs_roots.len(), 3);
    assert!(ctx1.fs_roots[1].replace('/', "\\").ends_with("ws\\a"));
    assert!(ctx1.fs_roots[2].replace('/', "\\").ends_with("ws\\b"));

    // 未归属：workspace=None + fs_roots 空（插件回落 env 同一集合语义）
    let ctx2 = resolver("session-2");
    assert_eq!(ctx2.workspace, None);
    assert!(ctx2.fs_roots.is_empty());
    // 非法 id / 缺旁车 fail-closed
    assert_eq!(resolver("../escape").workspace, None);
    assert_eq!(resolver("ghost").workspace, None);
}

#[test]
fn fence_reflects_current_settings_on_each_call() {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-ws-fence-live-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    session_store::write_workspace(&dir, "s1", Some("F:/ws/a")).unwrap();
    // 供给可变：设置保存即生效（下一次调用读新值）
    let roots = Arc::new(std::sync::Mutex::new(vec!["F:/ws/a".to_string()]));
    let shadow = Arc::clone(&roots);
    let resolver = session_context_resolver(
        dir.clone(),
        dir.join("data"),
        Arc::new(move || shadow.lock().unwrap().clone()),
    );
    assert_eq!(resolver("s1").fs_roots.len(), 2);
    *roots.lock().unwrap() = vec!["F:/ws/a".into(), "F:/ws/b".into(), "F:/ws/c".into()];
    assert_eq!(resolver("s1").fs_roots.len(), 4, "保存即生效，无需重建");
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
    let view = workspace_view(session_context_resolver(
        dir.clone(),
        dir.join("data"),
        no_roots(),
    ));
    assert_eq!(view("session-1").as_deref(), Some("F:/ws/a"));
    assert_eq!(view("ghost"), None);
}

#[test]
fn scope_key_normalizes_to_unfiled() {
    assert_eq!(scope_key(None), "");
    assert_eq!(scope_key(Some("")), "");
    assert_eq!(scope_key(Some("F:/ws/a")), "F:/ws/a");
}
