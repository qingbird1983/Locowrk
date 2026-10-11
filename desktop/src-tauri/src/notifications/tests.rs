//! notifications 单测：store 持久化 / 上限 / 已读语义 + draft_from_event
//! 五类提取与 ToolCall↔ToolResult 配对（纯函数，无需 Tauri 环境）。

use super::*;
use crate::events::AgentEvent;
use serde_json::json;

fn tmp_store(tag: &str) -> (NotificationStore, PathBuf) {
    let dir = crate::testutil::work_dir(&format!("lw-notif-test-{}", tag));
    let path = dir.join("notifications.json");
    (NotificationStore::open(path.clone()), path)
}

fn call(tool: &str, id: &str) -> AgentEvent {
    AgentEvent::ToolCall {
        session_id: "s1".into(),
        call_id: id.into(),
        tool: tool.into(),
        args: json!(null),
        side_effect: "pure".into(),
        plugin_version: "1.0.0".into(),
    }
}

fn result(id: &str, ok: bool, error: Option<&str>) -> AgentEvent {
    result_with(id, ok, json!(null), error)
}

fn result_with(id: &str, ok: bool, output: serde_json::Value, error: Option<&str>) -> AgentEvent {
    AgentEvent::ToolResult {
        session_id: "s1".into(),
        call_id: id.into(),
        ok,
        output,
        error: error.map(Into::into),
    }
}

#[test]
fn add_list_persist_roundtrip_and_cap() {
    let (store, path) = tmp_store("roundtrip");
    store.add(NotificationKind::SubagentDone, Some("s1".into()), None);
    store.add(
        NotificationKind::ScheduleFired,
        Some("s2".into()),
        Some("跑批".into()),
    );
    let list = store.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].kind, NotificationKind::ScheduleFired, "新→旧");
    assert_eq!(list.iter().filter(|i| !i.read).count(), 2, "新入账全未读");

    // 重开同一文件 = 持久恢复（新→旧：「跑批」最新在 [0]）
    let reopened = NotificationStore::open(path.clone());
    assert_eq!(reopened.list().len(), 2);
    assert_eq!(reopened.list()[0].label.as_deref(), Some("跑批"));

    // FIFO 上限：超出 CAP 删最旧
    for i in 0..(CAP as u64 + 10) {
        reopened.add(
            NotificationKind::ContextCompacted,
            None,
            Some(i.to_string()),
        );
    }
    assert_eq!(reopened.list().len(), CAP);
    let kept = reopened.list();
    let oldest_kept = kept.last().unwrap();
    assert_eq!(oldest_kept.label.as_deref(), Some("10"), "最旧 10 条被删");
    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn mark_read_semantics_and_clear() {
    let (store, _path) = tmp_store("mark");
    let unread = || store.list().iter().filter(|i| !i.read).count();
    let a = store.add(NotificationKind::SubagentDone, None, None);
    store.add(NotificationKind::ApprovalTimeout, None, None);
    assert!(store.mark_read(&a.id));
    assert_eq!(unread(), 1);
    assert!(!store.mark_read("no-such-id"));
    assert_eq!(store.mark_all_read(), 1);
    assert_eq!(unread(), 0);
    assert_eq!(store.mark_all_read(), 0, "已全读再置零条");
    store.clear();
    assert!(store.list().is_empty());
    std::fs::remove_dir_all(_path.parent().unwrap()).ok();
}

#[test]
fn draft_extracts_five_kinds_and_pairing() {
    let mut pending = HashMap::new();
    // 子 Agent 成功收束：配对出通知（output.status = completed 才算完成）
    assert!(draft_from_event(&call("subagent_run", "c1"), &mut pending).is_none());
    let d = draft_from_event(
        &result_with("c1", true, json!({"status": "completed"}), None),
        &mut pending,
    )
    .unwrap();
    assert_eq!(d.kind, NotificationKind::SubagentDone);
    assert_eq!(d.session_id.as_deref(), Some("s1"));
    // 工具闭合成功但子任务 failed/预算耗尽 → 如实入账为失败（走查实机抓到）
    draft_from_event(&call("subagent_run", "c5"), &mut pending);
    let d = draft_from_event(
        &result_with("c5", true, json!({"status": "failed"}), None),
        &mut pending,
    )
    .unwrap();
    assert_eq!(d.kind, NotificationKind::SubagentFailed);
    // 失败收束：kind 换 + 错误摘要进 label
    draft_from_event(&call("subagent_run", "c2"), &mut pending);
    let d = draft_from_event(&result("c2", false, Some("预算耗尽")), &mut pending).unwrap();
    assert_eq!(d.kind, NotificationKind::SubagentFailed);
    assert_eq!(d.label.as_deref(), Some("预算耗尽"));
    // 非子 Agent 工具与无配对的 ToolResult 都不入账
    draft_from_event(&call("shell", "c3"), &mut pending);
    assert!(draft_from_event(&result("c3", true, None), &mut pending).is_none());
    assert!(draft_from_event(&result("c9", true, None), &mut pending).is_none());

    let d = draft_from_event(
        &AgentEvent::ScheduleFired {
            session_id: "s1".into(),
            schedule_id: "sc1".into(),
            title: "每日站会摘要".into(),
        },
        &mut pending,
    )
    .unwrap();
    assert_eq!(d.kind, NotificationKind::ScheduleFired);
    assert_eq!(d.label.as_deref(), Some("每日站会摘要"));

    let d = draft_from_event(
        &AgentEvent::ContextCompacted {
            session_id: "s1".into(),
            from_tokens: 10_000,
            to_tokens: 4_000,
            strategy: "sliding_window".into(),
            summary_preview: String::new(),
        },
        &mut pending,
    )
    .unwrap();
    assert_eq!(d.kind, NotificationKind::ContextCompacted);
    assert_eq!(d.label.as_deref(), Some("6000"));

    // 审批超时拒绝（共享前缀常量命中）入账；手动拒绝 / 放行不入账
    let d = draft_from_event(
        &AgentEvent::ApprovalResolved {
            session_id: "s1".into(),
            call_id: "c4".into(),
            tool: "shell".into(),
            allowed: false,
            reason: Some(format!(
                "{TIMEOUT_REASON_PREFIX}（100 秒未响应，fail-closed）"
            )),
        },
        &mut pending,
    )
    .unwrap();
    assert_eq!(d.kind, NotificationKind::ApprovalTimeout);
    assert!(draft_from_event(
        &AgentEvent::ApprovalResolved {
            session_id: "s1".into(),
            call_id: "c4".into(),
            tool: "shell".into(),
            allowed: false,
            reason: Some("用户拒绝".into()),
        },
        &mut pending
    )
    .is_none());
    assert!(draft_from_event(
        &AgentEvent::ApprovalResolved {
            session_id: "s1".into(),
            call_id: "c4".into(),
            tool: "shell".into(),
            allowed: true,
            reason: None,
        },
        &mut pending
    )
    .is_none());
}
