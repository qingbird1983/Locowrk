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
    let d = std::env::temp_dir().join(format!("locowrk-slog-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
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
        None,
        &sink,
        None,
        Some(&dir),
        crate::runner::DEFAULT_MAX_STEPS,
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
        None,
        &sink,
        None,
        Some(&dir),
        crate::runner::DEFAULT_MAX_STEPS,
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

#[test]
fn undo_rewrite_truncates_and_keeps_backup() {
    let dir = tmp_dir("undo");
    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "一".into(),
    });
    session.append(SessionEvent::UserMessage {
        content: "二".into(),
    });
    session_log::append_slice(&dir, "s", &session, 0);

    // undo 语义：fork 截断 → rewrite_after_undo
    session.rewind_to(0);
    let mut forked = session.fork(0);
    forked.append(SessionEvent::UserMessage {
        content: "新轮".into(),
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
        });
        s.append(SessionEvent::AssistantMessage {
            content: "第一答".into(),
            tool_calls: vec![],
        });
        s.append(SessionEvent::UserMessage {
            content: "第二问".into(),
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

/// v2.62 §3.12 上下文统计持久化：run_turn（带 UsagePolicy + log）把
/// `SessionEvent::ContextUsage` 落日志（轮次前 + 每 step 后各一条），
/// 重启恢复后全量投影含同形事件且逐字段透传——切会话回来用量环恢复
/// （此前快照只推实时通道，重放面没有该事件，切回来统计即消失）。
#[tokio::test]
async fn context_usage_persists_and_replays() {
    let dir = tmp_dir("ctx-usage");
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s-ctx",
        "echo 你好".into(),
        Some(crate::runner::UsagePolicy {
            threshold_tokens: 8000,
            window_tokens: 32000,
            tools_est_tokens: 512,
        }),
        &sink,
        None,
        Some(&dir),
        crate::runner::DEFAULT_MAX_STEPS,
    )
    .await
    .expect("turn ok");

    // 实时面：进度条数据源照常推送
    assert!(sink
        .events()
        .iter()
        .any(|e| matches!(e, AgentEvent::ContextUsage { .. })));

    // 重启恢复面：全量投影含同形快照（轮次前 + 每 step 后 ≥ 2 条）
    let map: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    assert_eq!(session_log::restore_all(&dir, &map), 1);
    let restored = map.lock().unwrap().get("s-ctx").cloned().unwrap();
    let restored = restored.lock().await;
    let projected = crate::runner::project_events(restored.events(), "s-ctx");
    let hits: Vec<&AgentEvent> = projected
        .iter()
        .filter(|e| matches!(e, AgentEvent::ContextUsage { .. }))
        .collect();
    assert!(
        hits.len() >= 2,
        "轮次前 + 每 step 后各一条快照，实际 {} 条",
        hits.len()
    );
    match hits.last() {
        Some(AgentEvent::ContextUsage {
            used,
            threshold,
            window_tokens,
            tools_est_tokens,
            ..
        }) => {
            assert_eq!(*threshold, 8000);
            assert_eq!(*window_tokens, 32000);
            assert_eq!(*tools_est_tokens, 512);
            assert!(*used > 0, "估认真发生");
        }
        other => panic!("末条投影应为 ContextUsage：{:?}", other.is_none()),
    }
    std::fs::remove_dir_all(&dir).ok();
}
