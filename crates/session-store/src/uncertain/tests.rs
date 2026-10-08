//! G2 扫描与合成的回归面：悬空两形态（ToolResult 缺席 / ToolCall 事件
//! 缺席）、裁决排除、投影闭合、closure 载荷形状。

use serde_json::json;

use super::*;
use plugin_api::CallId;

use crate::event::UncertainDecision;
use crate::{Message, Session, SessionEvent};

fn user(content: &str) -> SessionEvent {
    SessionEvent::UserMessage {
        content: content.into(),
        parts: Vec::new(),
    }
}

fn assistant(ids: &[&str]) -> SessionEvent {
    SessionEvent::AssistantMessage {
        content: String::new(),
        tool_calls: ids.iter().map(|s| CallId((*s).into())).collect(),
    }
}

fn call(id: &str) -> SessionEvent {
    SessionEvent::ToolCall {
        call_id: CallId(id.into()),
        tool: "fs_write".into(),
        args: json!({"path": "a.txt"}),
        plugin_id: None,
        plugin_version: semver::Version::new(0, 1, 0),
        side_effect: plugin_api::SideEffect::Compensable {
            undo_tool: String::new(),
        },
    }
}

fn result(id: &str) -> SessionEvent {
    SessionEvent::ToolResult {
        call_id: CallId(id.into()),
        output: json!({"ok": true}),
        error: None,
    }
}

fn decided(id: &str, d: UncertainDecision, note: &str) -> SessionEvent {
    SessionEvent::ToolExecutionUncertain {
        call_id: CallId(id.into()),
        decision: d,
        note: note.into(),
        ts: 1_700_000_000_000,
    }
}

#[test]
fn scan_finds_dangling_in_both_shapes() {
    // 形态一：ToolCall 事件已落、ToolResult 缺（执行中崩溃）
    let events = vec![
        user("hi"),
        assistant(&["a", "b"]),
        call("a"),
        // b 的 ToolCall 事件没来得及落（进程死在两次落盘之间）
    ];
    let scan = scan_uncertain(&events);
    assert!(scan.dangling.contains("a") && scan.dangling.contains("b"));
    let unresolved = scan.unresolved(&events);
    assert_eq!(unresolved.len(), 2);
    // a 有 ToolCall 事件 → 细节可回填；b 只有声明位 → 细节缺失如实
    assert_eq!(unresolved[0].tool, "fs_write");
    assert!(unresolved[1].tool.is_empty());
}

#[test]
fn scan_closed_and_decided_are_excluded_from_unresolved() {
    let events = vec![
        assistant(&["a", "b", "c"]),
        call("a"),
        result("a"),
        call("b"),
        decided("b", UncertainDecision::Completed, ""),
        call("c"),
    ];
    let scan = scan_uncertain(&events);
    assert_eq!(scan.dangling.len(), 2); // b、c 悬空（裁决不回填 ToolResult）
    let unresolved = scan.unresolved(&events);
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].call_id.0, "c");
    assert_eq!(
        scan.decisions.get("b").map(|(d, _)| *d),
        Some(UncertainDecision::Completed)
    );
}

#[test]
fn projection_closes_dangling_with_unknown_and_decision() {
    let mut s = Session::new();
    for e in [
        user("go"),
        assistant(&["a", "b"]),
        call("a"),
        call("b"),
        decided("a", UncertainDecision::NotExecuted, ""),
    ] {
        s.append(e);
    }
    let msgs = s.derive_messages();
    // user → assistant → 合成 a（裁决）→ 合成 b（未知）= 4 条
    assert_eq!(msgs.len(), 4);
    let Message::ToolResult {
        call_id,
        output,
        error,
        ..
    } = &msgs[2]
    else {
        panic!("expect synthesized ToolResult for a");
    };
    assert_eq!(call_id.0, "a");
    assert_eq!(output["status"], "confirmed_not_executed");
    assert_eq!(*error, None);
    let Message::ToolResult {
        call_id, output, ..
    } = &msgs[3]
    else {
        panic!("expect synthesized ToolResult for b");
    };
    assert_eq!(call_id.0, "b");
    assert_eq!(output["status"], "unknown");
    // 闭合紧贴 assistant 之后：下一轮请求形状合法（供应商不再拒收）
}

#[test]
fn projection_without_events_still_closes_declared_only_ids() {
    // ToolCall 事件全缺：只有 assistant 声明位也要闭合（细节空缺如实）
    let mut s = Session::new();
    s.append(assistant(&["x"]));
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 2);
    let Message::ToolResult { tool, output, .. } = &msgs[1] else {
        panic!("expect synthesized ToolResult for x");
    };
    assert!(tool.is_empty());
    assert_eq!(output["status"], "unknown");
}

#[test]
fn closed_calls_are_never_touched() {
    let mut s = Session::new();
    for e in [assistant(&["a"]), call("a"), result("a")] {
        s.append(e);
    }
    assert_eq!(s.derive_messages().len(), 2); // assistant + result，无合成
}

#[test]
fn closure_payload_shapes() {
    let (unknown, err) = closure(None);
    assert_eq!(unknown["status"], "unknown");
    assert_eq!(err, None);
    assert!(unknown.get("note").is_none());

    let (done, _) = closure(Some((UncertainDecision::Completed, "build/ 已确认删除")));
    assert_eq!(done["status"], "confirmed_completed");
    assert_eq!(done["note"], "build/ 已确认删除");

    let (done_empty, _) = closure(Some((UncertainDecision::Completed, "")));
    assert!(done_empty.get("note").is_none()); // 空附言不带字段

    let (inspected, _) = closure(Some((UncertainDecision::Inspected, "文件还在")));
    assert!(inspected["detail"].as_str().unwrap().contains("文件还在"));
    // inspected 的检查记录走 detail，不再重复 note 字段
    assert!(inspected.get("note").is_none());
}

#[test]
fn uncertain_event_serializes_snake_case_and_defaults() {
    let ev = decided("a", UncertainDecision::NotExecuted, "");
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["type"], "tool_execution_uncertain");
    assert_eq!(v["decision"], "not_executed");
    // 旧日志演进面：缺 note/ts 读入 = 缺省
    let bare = json!({
        "type": "tool_execution_uncertain",
        "call_id": "a",
        "decision": "inspected"
    });
    let back: SessionEvent = serde_json::from_value(bare).unwrap();
    assert!(matches!(
        back,
        SessionEvent::ToolExecutionUncertain { note, ts, .. } if note.is_empty() && ts == 0
    ));
}
