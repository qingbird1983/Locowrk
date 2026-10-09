//! tree_tools 单测：expand 摘要/原文切片 / 未知章节如实失败 / collapse
//! 只对已展开章节生效。（v2.137：legacy 拒绝面随整理模式退役——一切
//! 会话皆树；collapse 的暴露开关门控在 tool_source 侧，tests_tree_gate。）

use std::sync::Arc;

use plugin_api::{CallCtx, SideEffect, Tool};
use serde_json::json;
use session_store::{Session, SessionEvent};

use super::{MemoryCollapseTool, MemoryExpandTool};

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ttools-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|_sid: &str| Some(r"F:\ws\demo".to_string()))
}

/// 会话落盘（三轮问答）+ 两章节投影（b1 = [0..2) 两轮前的第一轮，b2 = [2..4)）。
/// 返回 (root, workspace)。
fn seed(root: &std::path::Path, ws: &str) {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题：关于主题{i}的细节"),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    std::fs::create_dir_all(root.join("sessions")).unwrap();
    session_store::rewrite_log(
        &session_store::session_file(&root.join("sessions"), "s1").unwrap(),
        &s,
    )
    .unwrap();

    let mut tree = memory_tree::MemoryTree::new(ws, "s1");
    let d1 = vec![memory_tree::sanitize_draft("章节一", "摘要一", (0, 1)).unwrap()];
    let b1 = tree.build_chapters(&d1, "recorder", 1);
    tree.apply_organized(
        b1,
        1,
        memory_tree::OrganizeRecord {
            ts: 1,
            reason: "manual".into(),
            mode: "recorder".into(),
            status: "ok".into(),
            from: 0,
            to: 1,
            chapters: 1,
            manager_input: 0,
            manager_output: 0,
            duration_ms: 1,
        },
    );
    let d2 = vec![memory_tree::sanitize_draft("章节二", "摘要二", (2, 3)).unwrap()];
    let b2 = tree.build_chapters(&d2, "recorder", 2);
    tree.apply_organized(
        b2,
        3,
        memory_tree::OrganizeRecord {
            ts: 2,
            reason: "manual".into(),
            mode: "recorder".into(),
            status: "ok".into(),
            from: 2,
            to: 3,
            chapters: 1,
            manager_input: 0,
            manager_output: 0,
            duration_ms: 1,
        },
    );
    memory_tree::TreeStore::new(root).save(&tree).unwrap();
}

fn ctx(session_id: Option<&str>) -> CallCtx {
    let mut c = CallCtx::new();
    c.session_id = session_id.map(str::to_string);
    c
}

fn expand(root: &std::path::Path) -> Arc<MemoryExpandTool> {
    MemoryExpandTool::new(
        memory_tree::TreeStore::new(root),
        resolver(),
        root.join("sessions"),
    )
}

#[tokio::test]
async fn expand_returns_summary_and_transcript() {
    let root = temp_root("expand");
    let ws = r"F:\ws\demo";
    seed(&root, ws);
    let tool = expand(&root);

    let v = tool
        .call(&ctx(Some("s1")), json!({ "chapter_id": "b1", "level": 1 }))
        .await
        .expect("level 1");
    assert_eq!(v["title"], "章节一");
    assert_eq!(v["summary"], "摘要一");

    let v = tool
        .call(&ctx(Some("s1")), json!({ "chapter_id": "b1", "level": 2 }))
        .await
        .expect("level 2");
    let t = v["transcript"].as_str().unwrap();
    assert!(t.contains("第0轮问题"), "原文切片含用户行");
    assert!(!t.contains("第2轮"), "不越过章节区间");

    // 展开记账落投影（自动收起的挑选依据）
    let tree = memory_tree::TreeStore::new(&root).load(ws, "s1").unwrap();
    assert!(tree.chapters["b1"].expanded_at.is_some());
}

#[tokio::test]
async fn expand_rejects_unknown_bundle() {
    let root = temp_root("reject");
    let ws = r"F:\ws\demo";
    seed(&root, ws);

    // 未知章节如实失败
    let tool = expand(&root);
    let err = tool
        .call(&ctx(Some("s1")), json!({ "chapter_id": "b9", "level": 1 }))
        .await
        .expect_err("未知章节拒绝");
    assert!(matches!(err, plugin_api::ToolError::Failed { .. }));

    // 规格纪律：expand = Pure + Always；collapse = Compensable + Hidden
    assert_eq!(tool.spec().side_effect, SideEffect::Pure);
    assert_eq!(tool.spec().exposure, plugin_api::Exposure::Always);
}

#[tokio::test]
async fn collapse_only_after_expand_and_expand_clears_it() {
    let root = temp_root("collapse");
    let ws = r"F:\ws\demo";
    seed(&root, ws);
    let expand_tool = expand(&root);
    let collapse_tool = MemoryCollapseTool::new(memory_tree::TreeStore::new(&root), resolver());
    assert_eq!(
        collapse_tool.spec().side_effect,
        SideEffect::Compensable {
            undo_tool: "memory_expand".into()
        }
    );
    assert_eq!(collapse_tool.spec().exposure, plugin_api::Exposure::Hidden);

    // 未展开不可收
    let err = collapse_tool
        .call(&ctx(Some("s1")), json!({ "chapter_id": "b1" }))
        .await
        .expect_err("未展开拒绝");
    assert!(matches!(err, plugin_api::ToolError::Failed { .. }));

    // 展开后再收起成功；再展开解除收起
    expand_tool
        .call(&ctx(Some("s1")), json!({ "chapter_id": "b1", "level": 1 }))
        .await
        .expect("展开");
    collapse_tool
        .call(&ctx(Some("s1")), json!({ "chapter_id": "b1" }))
        .await
        .expect("收起");
    let tree = memory_tree::TreeStore::new(&root).load(ws, "s1").unwrap();
    assert_eq!(tree.collapsed_ids(), vec!["b1".to_string()]);
    expand_tool
        .call(&ctx(Some("s1")), json!({ "chapter_id": "b1", "level": 1 }))
        .await
        .expect("再展开");
    let tree = memory_tree::TreeStore::new(&root).load(ws, "s1").unwrap();
    assert!(tree.collapsed_ids().is_empty(), "再展开解除收起");
}
