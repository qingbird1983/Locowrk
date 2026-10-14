//! ⑲ 会话落盘集成：run_turn 增量镜像 → 启动恢复一致；undo 截断重写 +
//! `.undo.bak` 备份留痕。落盘本体（JSONL 语义）在 session-store 的
//! persist 测试，这里锁组装点接线。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use session_store::{Session, SessionEvent};

use crate::events::AgentEvent;
use crate::runner::run_turn;
use crate::session_log;
use crate::tests::{kernel_with_tools, Collector};

fn tmp_dir(tag: &str) -> PathBuf {
    // 唯一 + 先清的契约见 `crate::testutil` 头注。本模块正是批 B 撞红的那处：
    // 只带 pid 的写法在 Windows 上会读到上次运行的残留（`.undo.bak` 读出 6 条 vs 期望 4）。
    crate::testutil::work_dir(&format!("slog-{}", tag))
}

#[tokio::test]
async fn turn_mirrors_to_disk_and_restores_identically() {
    let dir = tmp_dir("mirror");
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s-log",
        "echo 你好".into(),
        Vec::new(),
        None,
        &sink,
        None,
        Some(&dir),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");
    assert!(
        session.len() >= 3,
        "工具轮至少 user+toolcall+result+assistant"
    );

    // 启动恢复进空表：事件序列逐条一致
    let map: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    assert_eq!(session_log::restore_all(&dir, &map), 1);
    let restored = map.lock().unwrap().get("s-log").cloned().unwrap();
    let restored = restored.lock().await;
    assert_eq!(restored.len(), session.len());
    assert_eq!(restored.events(), session.events());
    std::fs::remove_dir_all(&dir).ok();
}

/// ㉕·fix 回归：重启后 `session_history` 全量投影必须含用户气泡——
/// 曾因 `project_events` 漏投影 `UserMessage`（落 `_` 通配），历史会话
/// 只剩模型回复。实时投影不推用户消息（前端发送即本地回显，防双气泡）。
#[tokio::test]
async fn history_projection_includes_user_messages() {
    let dir = tmp_dir("replay-user");
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s-replay",
        "echo 你好".into(),
        Vec::new(),
        None,
        &sink,
        None,
        Some(&dir),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    // 实时投影面：不出现用户消息（本地回显约定，双气泡即回归）
    assert!(!sink
        .events()
        .iter()
        .any(|e| matches!(e, AgentEvent::UserMessage { .. })));

    // 重启恢复面：全量投影首条 = 用户消息，正文原样
    let map: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    assert_eq!(session_log::restore_all(&dir, &map), 1);
    let restored = map.lock().unwrap().get("s-replay").cloned().unwrap();
    let restored = restored.lock().await;
    let projected = crate::runner::project_events(restored.events(), "s-replay");
    assert!(
        matches!(
            projected.first(),
            Some(AgentEvent::UserMessage { content, .. }) if content == "echo 你好"
        ),
        "全量投影首条应为用户消息：{:?}",
        projected.first()
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// G2 回归：悬空调用（有 ToolCall 无 ToolResult）的全量投影 = 工具行 +
/// 紧随的 tool_uncertain 标记；裁决后重放 = uncertain_resolved 收束且
/// 不再出卡。声明位无 ToolCall 事件的极端形态 = 空细节卡直接出。
#[test]
fn uncertain_projection_marks_dangling_and_resolves() {
    use plugin_api::CallId;
    use serde_json::json;

    let call = |id: &str| SessionEvent::ToolCall {
        call_id: CallId(id.into()),
        tool: "fs_write".into(),
        args: json!({"path": "a.txt"}),
        plugin_id: None,
        plugin_version: semver::Version::new(0, 1, 0),
        side_effect: plugin_api::SideEffect::Compensable {
            undo_tool: String::new(),
        },
    };
    let assistant = |ids: &[&str]| SessionEvent::AssistantMessage {
        content: String::new(),
        tool_calls: ids.iter().map(|s| CallId((*s).into())).collect(),
    };

    // 未裁决：工具行 + 卡
    let mut s = Session::new();
    for e in [
        SessionEvent::UserMessage {
            content: "go".into(),
            parts: Vec::new(),
        },
        assistant(&["a"]),
        call("a"),
    ] {
        s.append(e);
    }
    let projected = crate::runner::project_events(s.events(), "s-unc");
    let kinds: Vec<&str> = projected.iter().map(crate::tests::kind).collect();
    assert_eq!(
        kinds,
        vec!["user_message", "tool_call", "tool_uncertain"],
        "悬空调用应带 uncertain 卡：{kinds:?}"
    );

    // 裁决落日志后重放：复现「建卡→裁决收束→行闭合」的实时序列
    s.append(SessionEvent::ToolExecutionUncertain {
        call_id: CallId("a".into()),
        decision: session_store::UncertainDecision::Completed,
        note: String::new(),
        ts: 1,
    });
    let projected = crate::runner::project_events(s.events(), "s-unc");
    let kinds: Vec<&str> = projected.iter().map(crate::tests::kind).collect();
    assert_eq!(
        kinds,
        vec![
            "user_message",
            "tool_call",
            "tool_uncertain",
            "uncertain_resolved",
            "tool_result"
        ],
        "裁决后重放 = 建卡→收束→闭合：{kinds:?}"
    );
    let Some(AgentEvent::UncertainResolved { decision, .. }) = projected
        .iter()
        .find(|e| matches!(e, AgentEvent::UncertainResolved { .. }))
    else {
        panic!("应有 uncertain_resolved");
    };
    assert_eq!(decision, "completed");
    // 合成 ToolResult 按裁决闭合（ok=true + 状态字段）
    let Some(AgentEvent::ToolResult { ok, output, .. }) = projected.last() else {
        panic!("末条应为合成 tool_result");
    };
    assert!(*ok && output["status"] == "confirmed_completed");

    // 声明位无 ToolCall 事件：空细节卡挂在 assistant 段之后
    let mut s2 = Session::new();
    s2.append(assistant(&["ghost"]));
    let projected = crate::runner::project_events(s2.events(), "s-unc2");
    let kinds: Vec<&str> = projected.iter().map(crate::tests::kind).collect();
    assert_eq!(
        kinds,
        vec!["tool_uncertain"],
        "声明位悬空出空细节卡：{kinds:?}"
    );
    let Some(AgentEvent::ToolUncertain { tool, args, .. }) = projected.first() else {
        panic!("应为 tool_uncertain");
    };
    assert!(tool.is_empty() && args.is_null());
}

#[test]
fn undo_rewrite_truncates_and_keeps_backup() {
    let dir = tmp_dir("undo");
    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "一".into(),
        parts: Vec::new(),
    });
    session.append(SessionEvent::UserMessage {
        content: "二".into(),
        parts: Vec::new(),
    });
    session_log::append_slice(&dir, "s", &session, 0);

    // undo 语义：fork 截断 → rewrite_after_undo
    session.rewind_to(0);
    let mut forked = session.fork(0);
    forked.append(SessionEvent::UserMessage {
        content: "新轮".into(),
        parts: Vec::new(),
    });
    session_log::rewrite_after_undo(&dir, "s", &forked);

    // 恢复面：主文件 = fork 后日志；备份 = 撤销前全文
    let map: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    assert_eq!(session_log::restore_all(&dir, &map), 1);
    let restored = map.lock().unwrap().get("s").cloned().unwrap();
    assert_eq!(restored.blocking_lock().len(), 1);
    let bak = dir.join("s.jsonl.undo.bak");
    assert_eq!(
        session_log::list(&dir).len(),
        1,
        "备份不是 .jsonl，不进列表"
    );
    assert!(bak.exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn empty_and_invalid_sessions_are_skipped_on_restore() {
    let dir = tmp_dir("skip");
    // 空 fork 的重写 = 空文件 → 恢复跳过（空会话不进表）
    std::fs::write(dir.join("empty.jsonl"), "").unwrap();
    // 非法 id 文件不在列表（防穿越）；坏行文件恢复为前缀可用
    std::fs::write(dir.join("sub Dir.jsonl"), "").unwrap();
    std::fs::write(
        dir.join("half.jsonl"),
        "{\"type\":\"user_message\",\"content\":\"好\"}\n{oops\n",
    )
    .unwrap();

    let map: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    assert_eq!(session_log::restore_all(&dir, &map), 1, "只有 half 恢复");
    assert!(!map.lock().unwrap().contains_key("empty"));
    let s = map.lock().unwrap().get("half").cloned().unwrap();
    assert_eq!(s.blocking_lock().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

/// v2.57 §3.34 消息编辑重发：truncate_after 定位语义（Some=指定位必须
/// 是 UserMessage；None=最后一条）+ fork 截断 + 磁盘重写与 `.undo.bak`
/// 留痕。命令层只做锁与会话表存取，语义在这里锁定。
#[test]
fn truncate_after_resolves_pos_and_rewrites_disk() {
    let dir = tmp_dir("truncate");
    let sid = "desktop-truncate";
    // Session 不是 Clone（事件日志语义）：每例重造
    fn four_events() -> Session {
        let mut s = Session::new();
        s.append(SessionEvent::UserMessage {
            content: "第一问".into(),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: "第一答".into(),
            tool_calls: vec![],
        });
        s.append(SessionEvent::UserMessage {
            content: "第二问".into(),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: "第二答".into(),
            tool_calls: vec![],
        });
        s
    }
    let s = four_events();
    let path = session_store::session_file(&dir, sid).unwrap();
    for e in s.events() {
        session_store::append_event(&path, e).unwrap();
    }

    // Some(0)：截掉第一问起的全部（= 清空可见日志）
    let mut s2 = four_events();
    assert_eq!(session_log::truncate_after(&mut s2, Some(0)).unwrap(), 0);
    assert_eq!(s2.len(), 0);
    // Some(2)：保留第一轮，截掉第二问起的全部
    let mut s3 = four_events();
    assert_eq!(session_log::truncate_after(&mut s3, Some(2)).unwrap(), 2);
    assert_eq!(s3.len(), 2);
    assert!(matches!(
        s3.events().last(),
        Some(SessionEvent::AssistantMessage { content, .. }) if content == "第一答"
    ));
    // None：最后一条用户消息（与 Some(2) 等价）
    let mut s4 = four_events();
    assert_eq!(session_log::truncate_after(&mut s4, None).unwrap(), 2);
    assert_eq!(s4.len(), 2);
    // 非法位：越界 / 不是 UserMessage
    let mut s5 = four_events();
    assert!(session_log::truncate_after(&mut s5, Some(99)).is_err());
    let mut s6 = four_events();
    assert!(session_log::truncate_after(&mut s6, Some(1)).is_err());

    // 磁盘同步：截断后重写镜像，恢复一致；.undo.bak 留有截断前全文
    let mut s5 = four_events();
    session_log::truncate_after(&mut s5, None).unwrap();
    session_log::rewrite_after_undo(&dir, sid, &s5);
    let (restored, _) = session_store::load_session(&path).unwrap();
    assert_eq!(restored.len(), 2);
    let bak = path.with_file_name(format!("{sid}.jsonl.undo.bak"));
    let (prev, _) = session_store::load_session(&bak).unwrap();
    assert_eq!(prev.len(), 4, "备份保留截断前完整日志");
}

/// 上下文用量快照两测（v2.62 持久化 + v2.138 真实锚点）拆入子模块——
/// 母文件守 400 行纪律（compact/tests 先例）。
mod context_usage;
mod file_changes;
mod turn_header;
