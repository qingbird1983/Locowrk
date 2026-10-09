//! assemble 单测：树组装计划（边界 A/B / 目录排序与占位排除 / 收起清单）
//! + 组装后用量口径 + 窗口起点纯函数。

use std::sync::Arc;

use session_store::{Session, SessionEvent};

use kernel_agent::TreeContextSource as _;

use super::{window_start_event, TreeAssembler};
use crate::settings::SettingsStore;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("asm-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn settings_with(json: serde_json::Value, root: &std::path::Path) -> Arc<SettingsStore> {
    std::fs::write(root.join("settings.json"), json.to_string()).unwrap();
    Arc::new(SettingsStore::open(root).unwrap())
}

fn resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|_sid: &str| Some(r"F:\ws\demo".to_string()))
}

fn assembler(root: &std::path::Path, settings: Arc<SettingsStore>) -> TreeAssembler {
    TreeAssembler::new(
        settings,
        resolver(),
        memory_tree::TreeStore::new(root),
        root.join("sessions"),
    )
}

/// 十二轮问答（尾巴远小于全量，组装后口径可比的前提）。
fn long_session() -> Session {
    let mut s = Session::new();
    for i in 0..12 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题：关于主题{i}的细节"),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    s
}

fn record(to: usize) -> memory_tree::OrganizeRecord {
    memory_tree::OrganizeRecord {
        ts: 1,
        reason: "manual".into(),
        mode: "recorder".into(),
        status: "ok".into(),
        from: 0,
        to,
        chapters: 1,
        manager_input: 0,
        manager_output: 0,
        duration_ms: 1,
    }
}

/// 前置：十二轮会话写盘（模式 B 窗口起点要读盘）+ 前 20 个事件（十轮）
/// 归章节 b1（chaptered_until = 20）——尾巴只剩最后两轮。
fn seed_tree(root: &std::path::Path, ws: &str, session_id: &str) {
    let session = long_session();
    let dir = root.join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    session_store::rewrite_log(
        &session_store::session_file(&dir, session_id).unwrap(),
        &session,
    )
    .unwrap();
    let mut tree = memory_tree::MemoryTree::new(ws, session_id);
    let drafts = vec![memory_tree::sanitize_draft("章节一", "摘要一", (0, 19)).unwrap()];
    let chapters = tree.build_chapters(&drafts, "recorder", 1);
    tree.apply_organized(chapters, 19, record(19));
    memory_tree::TreeStore::new(root).save(&tree).unwrap();
}

#[tokio::test]
async fn tree_mode_a_slices_at_chaptered_until_and_lists_directory() {
    let root = temp_root("modea");
    let sid = "s1";
    let ws = r"F:\ws\demo";
    seed_tree(&root, ws, sid);
    let asm = assembler(&root, settings_with(serde_json::json!({}), &root));

    let plan = asm.plan(sid).await.expect("树会话有计划");
    assert_eq!(plan.tail_from_event, 20, "A 档 = chaptered_until 排他游标");
    assert!(plan.directory.starts_with("[memory tree]"), "稳定头行");
    assert!(plan.directory.contains("- b1: 章节一"), "章节清单行");
    assert!(!plan.directory.contains("</"), "无闭合标签（追加不破前缀）");
    assert!(plan.collapsed_chapters.is_empty());

    // 组装后口径 = 目录 + 尾巴四条消息，显著小于全量二十四条
    let s = long_session();
    let tokens = asm.context_tokens(&s, sid).expect("树会话给用量口径");
    let full = kernel_agent::estimate_tokens(&s.derive_messages());
    assert!(tokens < full / 2, "组装后 {tokens} << 全量 {full}");
    assert!(tokens > 0);
}

#[tokio::test]
async fn tree_mode_b_window_floors_at_chaptered_until() {
    let root = temp_root("modeb");
    let sid = "s1";
    let ws = r"F:\ws\demo";
    seed_tree(&root, ws, sid);
    let asm = assembler(
        &root,
        settings_with(
            serde_json::json!({ "memory": { "tailStrategy": "window", "tailWindowTurns": 1 } }),
            &root,
        ),
    );
    // 窗口 1 轮：起点 = 最后一条 User（事件 22）；chaptered_until=20 → from=22
    let plan = asm.plan(sid).await.expect("计划在场");
    assert_eq!(plan.tail_from_event, 22, "窗口起点在游标之后取窗口起点");

    // 窗口 30 轮（大于既有轮数）：起点 = 0，但被游标抬到 20（不重复组装）
    let asm5 = assembler(
        &root,
        settings_with(
            serde_json::json!({ "memory": { "tailStrategy": "window", "tailWindowTurns": 30 } }),
            &root,
        ),
    );
    let plan5 = asm5.plan(sid).await.expect("计划在场");
    assert_eq!(plan5.tail_from_event, 20, "max(chaptered_until, 窗口起点)");
}

#[tokio::test]
async fn pending_bundles_never_enter_directory() {
    let root = temp_root("pending");
    let sid = "s1";
    let ws = r"F:\ws\demo";
    seed_tree(&root, ws, sid);
    let mut tree = memory_tree::TreeStore::new(&root).load(ws, sid).unwrap();
    tree.insert_placeholder(20, 21, 9).expect("占位可建");
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();

    let asm = assembler(&root, settings_with(serde_json::json!({}), &root));
    let plan = asm.plan(sid).await.expect("计划在场");
    assert!(!plan.directory.contains("对话 #"), "占位章节不进目录");
    assert!(plan.directory.contains("b1"));
}

#[tokio::test]
async fn collapsed_ids_flow_into_plan() {
    let root = temp_root("collapsed");
    let sid = "s1";
    let ws = r"F:\ws\demo";
    seed_tree(&root, ws, sid);
    let mut tree = memory_tree::TreeStore::new(&root).load(ws, sid).unwrap();
    tree.mark_expanded("b1", 5);
    tree.mark_collapsed("b1", 6);
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();

    let asm = assembler(&root, settings_with(serde_json::json!({}), &root));
    let plan = asm.plan(sid).await.expect("计划在场");
    assert_eq!(
        plan.collapsed_chapters,
        vec!["b1".to_string()],
        "收起清单随计划交给内核"
    );
}

#[test]
fn window_start_event_picks_nth_last_user() {
    let mk = |n: usize| {
        let mut s = Session::new();
        for i in 0..n {
            s.append(SessionEvent::UserMessage {
                content: format!("u{i}"),
                parts: Vec::new(),
            });
            s.append(SessionEvent::AssistantMessage {
                content: format!("a{i}"),
                tool_calls: vec![],
            });
        }
        s.events().to_vec()
    };
    let events = mk(3);
    assert_eq!(window_start_event(&events, 1), 4, "最后一轮起点");
    assert_eq!(window_start_event(&events, 2), 2);
    assert_eq!(window_start_event(&events, 5), 0, "不足 N = 全部");
    assert_eq!(window_start_event(&[], 3), 0, "空会话安全");
}
