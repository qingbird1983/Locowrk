//! `session` 模块单测：两遍投影回填、游标回退、fork 快照裁剪、快照折算。

use semver::Version;
use serde_json::json;

use plugin_api::{CallId, SideEffect};

use super::{SNAPSHOT_EVERY, Session};
use crate::event::SessionEvent;

fn tool_call(seq: u32) -> SessionEvent {
    SessionEvent::ToolCall {
        call_id: CallId(format!("call-{seq}")),
        tool: "echo".into(),
        args: json!({ "text": "hi" }),
        plugin_version: Version::new(0, 1, 0),
        side_effect: SideEffect::Pure,
    }
}

#[test]
fn derive_backfills_tool_name_and_args() {
    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "hi via tool".into(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "need tool".into(),
        tool_calls: vec![CallId("call-1".into())],
    });
    s.append(tool_call(1));
    s.append(SessionEvent::ToolResult {
        call_id: CallId("call-1".into()),
        output: json!({ "echo": "hi" }),
        error: None,
    });

    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 3);
    let crate::Message::Assistant { tool_calls, .. } = &msgs[1] else {
        panic!("expected assistant message");
    };
    // AssistantMessage 只存 CallId；工具名与参数从 ToolCall 回填（§4.2）
    assert_eq!(tool_calls[0].tool, "echo");
    assert_eq!(tool_calls[0].args, json!({ "text": "hi" }));
    let crate::Message::ToolResult { tool, args, .. } = &msgs[2] else {
        panic!("expected tool result message");
    };
    assert_eq!((tool.as_str(), args), ("echo", &json!({ "text": "hi" })));
}

#[test]
fn rewind_moves_cursor_and_keeps_log() {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(SessionEvent::UserMessage {
            content: i.to_string(),
        });
    }
    s.rewind_to(1);
    assert_eq!(s.cursor(), 1);
    assert_eq!(s.derive_messages().len(), 1);
    // 事件只追加不删除：日志仍是 3 条
    assert_eq!(s.len(), 3);
}

#[test]
fn fork_copies_prefix_and_clips_snapshots() {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(SessionEvent::UserMessage {
            content: i.to_string(),
        });
    }
    s.rewind_to(1);
    let forked = s.fork(1);
    assert_eq!(forked.len(), 1);
    assert_eq!(forked.cursor(), 1);
    assert_eq!(forked.derive_messages().len(), 1);
}

#[test]
fn rewind_before_tool_call_lands_on_toolcall_index() {
    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "go".into(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: String::new(),
        tool_calls: vec![CallId("call-0".into())],
    });
    s.append(tool_call(0));
    assert_eq!(s.rewind_before(&CallId("call-0".into())), Some(2));
    assert_eq!(s.cursor(), 2);
    assert_eq!(s.rewind_before(&CallId("missing".into())), None);
}

#[test]
fn state_at_folds_from_snapshot_base() {
    let mut s = Session::new();
    for i in 0..(SNAPSHOT_EVERY * 2 + 2) {
        s.append(SessionEvent::UserMessage {
            content: i.to_string(),
        });
    }
    // 内部快照落在 64 / 128
    assert!(s.state_at(SNAPSHOT_EVERY).messages.len() == SNAPSHOT_EVERY);
    let total = SNAPSHOT_EVERY * 2 + 2;
    assert_eq!(s.state_at(total).messages.len(), total);
    // 快照只保留 <= seq 的（fork 后 state_at 不越界，§4.2）
    let forked = s.fork(SNAPSHOT_EVERY + 1);
    assert_eq!(
        forked.state_at(SNAPSHOT_EVERY + 1).messages.len(),
        SNAPSHOT_EVERY + 1
    );
}
