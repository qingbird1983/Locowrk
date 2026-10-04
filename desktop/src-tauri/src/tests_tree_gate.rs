//! M4 树工具目录门控组装测试：Tree 档会话的目录含 memory_expand
//! （exposure=always）、collapse 开关开启才含；legacy 档两者皆无。

use std::sync::Arc;

use kernel_agent::ToolSource;
use plugin_api::Exposure;
use plugin_host::RegistryToolSource;

use crate::organizer_mode::{OrganizerMode, OrganizerModeStore};
use crate::tool_source::{KernelToolSource, TreeToolGate};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("treegate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn source(dir: &std::path::Path, default: OrganizerMode, collapse: bool) -> KernelToolSource {
    let modes = Arc::new(OrganizerModeStore::new(dir.join("sessions"), default));
    let root = dir.to_path_buf();
    let ws: crate::workspace::WorkspaceResolver = Arc::new(|_| None);
    KernelToolSource::new(RegistryToolSource::new(Arc::new(
        plugin_host::PluginRegistry::new(),
    )))
    .with_builtin(crate::organize::MemoryExpandTool::new(
        memory_tree::TreeStore::new(&root),
        Arc::clone(&ws),
        Arc::clone(&modes),
        dir.join("sessions"),
    ))
    .with_builtin(crate::organize::MemoryCollapseTool::new(
        memory_tree::TreeStore::new(&root),
        ws,
        Arc::clone(&modes),
    ))
    .with_tree_gate(Arc::new(TreeToolGate::new(
        modes,
        Arc::new(move || collapse),
    )))
}

#[tokio::test]
async fn tree_session_directory_contains_expand_not_collapse() {
    let dir = temp_dir("tree");
    let src = source(&dir, OrganizerMode::Tree, false);
    let names: Vec<String> = src
        .list(Some("s1"))
        .await
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(
        names.contains(&"memory_expand".to_string()),
        "树会话目录应含 expand：{names:?}"
    );
    assert!(
        !names.contains(&"memory_collapse".to_string()),
        "collapse 缺省关"
    );
    let spec = src
        .resolve(Some("s1"), "memory_expand")
        .await
        .expect("expand 可解析");
    assert_eq!(spec.spec().exposure, Exposure::Always);
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn legacy_session_directory_hides_tree_tools() {
    let dir = temp_dir("legacy");
    let src = source(&dir, OrganizerMode::Legacy, true);
    let names: Vec<String> = src
        .list(Some("s1"))
        .await
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(
        !names.contains(&"memory_expand".to_string()),
        "legacy 目录不含 expand：{names:?}"
    );
    assert!(
        !names.contains(&"memory_collapse".to_string()),
        "legacy 目录不含 collapse（即使开关开）"
    );
    let err = src
        .resolve(Some("s1"), "memory_expand")
        .await
        .err()
        .expect("legacy 解析应被拒");
    assert!(matches!(err, plugin_api::ToolError::NotAvailable(_)));
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn collapse_switch_exposes_for_tree_sessions() {
    let dir = temp_dir("collapse");
    let src = source(&dir, OrganizerMode::Tree, true);
    let names: Vec<String> = src
        .list(Some("s1"))
        .await
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(
        names.contains(&"memory_collapse".to_string()),
        "开关开 = 进目录"
    );
    std::fs::remove_dir_all(&dir).ok();
}
