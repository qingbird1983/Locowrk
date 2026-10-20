//! assemble 单测：树组装计划（边界 A/B / 目录排序与占位排除 / 收起清单）
//! + 组装后用量口径 + 窗口起点纯函数。

use std::sync::Arc;

use session_store::{Session, SessionEvent};

use kernel_agent::TreeContextSource as _;

use super::{window_start_event, TreeAssembler};
use crate::settings::SettingsStore;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = crate::testutil::work_dir(&format!("asm-{}", tag));
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
    tree.apply_organized(chapters, 19, record(19), None);
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
    // Q2（MR §4.2）：进展行拼在目录**之前**（项目级读数在前、本会话目录在后）。
    // 目录自己的稳定头行仍在原位——只是不再是整个 directory 的第一个字符，
    // 所以这里按"位置关系"判而不是"以谁开头"判。
    let at = plan.directory.find("[memory tree]").expect("目录头行在场");
    assert!(
        plan.directory[..at].starts_with("[project memory]"),
        "进展行必须在目录段之前: {:?}",
        &plan.directory[..at.min(40)]
    );
    assert!(
        plan.directory[at..].starts_with("[memory tree]"),
        "稳定头行"
    );
    assert!(plan.directory.contains("- b1: 章节一"), "章节清单行");
    assert!(!plan.directory.contains("</"), "无闭合标签（追加不破前缀）");
    assert!(plan.collapsed_chapters.is_empty());
    // ⚠ 这里原来还有一段"`context_tokens` 组装后口径 = 目录 + 尾巴 < 全量"的断言
    // （判进度条读数显著低于全量历史）。S3 刀④a 连方法带断言一起摘：进度条不再
    // 有第二把尺（分子统一 `context_budget::occupancy`），留着就是给一个没有生产
    // 调用点的 API 供读数——"存在但从未接线"正是本仓取证点名的形状。方法摘除的
    // 理由写在 `assemble.rs` 头注那条墓碑。
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

// ---- Q2（MR §4.2）：进展行走生产 `plan` 的接线判据（钉死 + 换归属重算） ----

/// 可改归属的解析器（会话中途被用户关联到另一个项目 = 生产里的真实动作）。
fn mutable_resolver() -> (
    Arc<std::sync::Mutex<String>>,
    crate::workspace::WorkspaceResolver,
) {
    let cur = Arc::new(std::sync::Mutex::new(r"F:\ws\demo".to_string()));
    let flip = Arc::clone(&cur);
    let r: crate::workspace::WorkspaceResolver =
        Arc::new(move |_| Some(flip.lock().unwrap().clone()));
    (cur, r)
}

#[tokio::test]
async fn progress_line_is_frozen_per_session_and_recomputes_on_reattribution() {
    // 稿 §4.5「会话内二次请求字节不变」：**必须走生产 `plan`** 才算接线证明——
    // 只在 `progress/tests.rs` 里测纯函数，测不到"每 step 都调 plan 而字节不变"。
    let root = temp_root("progpin");
    let sid = "s1";
    seed_tree(&root, r"F:\ws\demo", sid);
    let settings = settings_with(serde_json::json!({}), &root);
    let (cur, resolver) = mutable_resolver();
    let asm = TreeAssembler::new(
        settings,
        resolver,
        memory_tree::TreeStore::new(&root),
        root.join("sessions"),
    );

    let first = asm.plan(sid).await.expect("有计划").directory;
    assert!(
        first.contains("[project memory]"),
        "首请求就该带进展行: {first}"
    );

    // 中途整理产出**第二棵树的章节**（同项目另一个会话）：本会话已注入的字节
    // 一个字都不许变——变了就打断前缀缓存（KV v2 纪律 2，M1 的成因）。
    let mut extra = memory_tree::MemoryTree::new(r"F:\ws\demo", "s2");
    let drafts = vec![memory_tree::sanitize_draft("中途新章", "摘要", (0, 5)).unwrap()];
    let chapters = extra.build_chapters(&drafts, "recorder", 2);
    extra.apply_organized(chapters, 5, record(5), None);
    memory_tree::TreeStore::new(&root).save(&extra).unwrap();

    let second = asm.plan(sid).await.expect("有计划").directory;
    assert_eq!(first, second, "会话中途产新章不许改写已注入字节");

    // 改归属 = 键位变 = 换钉死键重算（新项目的进展行，不能沿用旧项目的读数）
    *cur.lock().unwrap() = r"F:\ws\other".to_string();
    let third = asm.plan(sid).await.expect("有计划").directory;
    assert!(
        !third.contains("[project memory]"),
        "换到无树的项目后进展行应整段缺席: {third}"
    );
}

#[tokio::test]
async fn unattributed_session_gets_no_progress_line_but_keeps_the_directory() {
    // 未归属（键位空串）= 「这个项目」不成立：目录段照旧、进展行一段都不注。
    let root = temp_root("progunattr");
    let sid = "s1";
    seed_tree(&root, r"F:\ws\demo", sid);
    let settings = settings_with(serde_json::json!({}), &root);
    let none: crate::workspace::WorkspaceResolver = Arc::new(|_| None);
    let asm = TreeAssembler::new(
        settings,
        none,
        memory_tree::TreeStore::new(&root),
        root.join("sessions"),
    );
    let plan = asm.plan(sid).await.expect("有计划");
    assert!(
        !plan.directory.contains("[project memory]"),
        "未归属不注进展行: {}",
        plan.directory
    );
    // 未归属会话的树本来就不在这个键位下 ⇒ 目录也是空的（`TreeDirectoryInject`
    // 对空串不追加）；这里不重复判目录形状——那由 `tree_mode_a_…` 那条管。
    assert!(
        plan.directory.is_empty(),
        "未归属会话无目录段: {}",
        plan.directory
    );
    assert_eq!(plan.tail_from_event, 0);
}
