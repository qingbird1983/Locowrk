//! M4 树工具目录门控组装测试（v2.137 收敛）：一切会话皆树（legacy 整理
//! 面退役），expand 常驻进目录；collapse 只看暴露开关（缺省关）。

use std::sync::Arc;

use kernel_agent::ToolSource;
use plugin_api::Exposure;
use plugin_host::RegistryToolSource;

use crate::tool_source::{KernelToolSource, TreeToolGate};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("treegate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn source(dir: &std::path::Path, collapse: bool) -> KernelToolSource {
    let root = dir.to_path_buf();
    let ws: crate::workspace::WorkspaceResolver = Arc::new(|_| None);
    KernelToolSource::new(RegistryToolSource::new(Arc::new(
        plugin_host::PluginRegistry::new(),
    )))
    .with_builtin(crate::organize::MemoryExpandTool::new(
        memory_tree::TreeStore::new(&root),
        Arc::clone(&ws),
        dir.join("sessions"),
    ))
    .with_builtin(crate::organize::MemoryCollapseTool::new(
        memory_tree::TreeStore::new(&root),
        ws,
    ))
    .with_tree_gate(Arc::new(TreeToolGate::new(Arc::new(move || collapse))))
}

#[tokio::test]
async fn directory_contains_expand_not_collapse_by_default() {
    let dir = temp_dir("tree");
    let src = source(&dir, false);
    let names: Vec<String> = src
        .list(Some("s1"))
        .await
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(
        names.contains(&"memory_expand".to_string()),
        "目录应含 expand：{names:?}"
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
async fn collapse_switch_exposes_tool() {
    let dir = temp_dir("collapse");
    let src = source(&dir, true);
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
    src.resolve(Some("s1"), "memory_collapse")
        .await
        .expect("开关开 = 可解析");
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn collapse_switch_off_rejects_resolve() {
    let dir = temp_dir("collapse-off");
    let src = source(&dir, false);
    let err = src
        .resolve(Some("s1"), "memory_collapse")
        .await
        .err()
        .expect("开关关 = 解析被拒");
    assert!(matches!(err, plugin_api::ToolError::NotAvailable(_)));
    std::fs::remove_dir_all(&dir).ok();
}
