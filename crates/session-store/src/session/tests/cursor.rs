//! 游标与快照面（自 `tests.rs` 批 F-23 拆出，用例本体逐字未改）：回退只移动游标、
//! 日志不删 / fork 复制前缀并裁剪快照 / `rewind_before` 落在 ToolCall 序号 /
//! `state_at` 从内部快照起步折算。

use super::*;

#[test]
fn rewind_moves_cursor_and_keeps_log() {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(user(i.to_string()));
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
        s.append(user(i.to_string()));
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
    s.append(user("go"));
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
        s.append(user(i.to_string()));
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
