//! `session` 模块单测：两遍投影回填、游标回退、fork 快照裁剪、快照折算、
//! parts 穿透（批 D）。

use semver::Version;
use serde_json::json;

use plugin_api::{CallId, SideEffect};

use super::{SNAPSHOT_EVERY, Session};
use crate::content::{AssetRef, ContentPart};
use crate::event::SessionEvent;

fn user(content: impl Into<String>) -> SessionEvent {
    SessionEvent::UserMessage {
        content: content.into(),
        parts: Vec::new(),
    }
}

fn tool_call(seq: u32) -> SessionEvent {
    SessionEvent::ToolCall {
        call_id: CallId(format!("call-{seq}")),
        tool: "echo".into(),
        args: json!({ "text": "hi" }),
        plugin_id: None,
        plugin_version: Version::new(0, 1, 0),
        side_effect: SideEffect::Pure,
    }
}

fn img(name: &str) -> ContentPart {
    ContentPart::Image {
        asset: AssetRef {
            id: format!("sha-{name}"),
            mime: "image/png".into(),
            name: name.into(),
        },
    }
}

#[test]
fn derive_backfills_tool_name_and_args() {
    let mut s = Session::new();
    s.append(user("hi via tool"));
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

// ---- ContextCompacted 折叠（Context and Memory.md §3.7，⑨） ----

fn compacted(kept: usize, summary: &str) -> SessionEvent {
    SessionEvent::ContextCompacted {
        kept_messages: kept,
        summary: summary.into(),
        from_tokens: 1000,
        to_tokens: 400,
        strategy: "layered".into(),
        ts: 0,
    }
}

#[test]
fn compaction_folds_into_summary_plus_kept_tail() {
    let mut s = Session::new();
    for i in 0..4 {
        s.append(user(format!("q{i}")));
    }
    let seq = s.append(compacted(2, "earlier notes"));
    assert!(seq > 0);

    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 3, "摘要 1 条 + 保留尾部 2 条");
    let crate::Message::User { content, .. } = &msgs[0] else {
        panic!("摘要必须是 User 消息");
    };
    assert!(content.contains("earlier notes"));
    // 尾部 = 最后 kept 条原文（q2、q3）
    assert_eq!(
        msgs[1..]
            .iter()
            .map(|m| match m {
                crate::Message::User { content, .. } => content.as_str(),
                _ => "",
            })
            .collect::<Vec<_>>(),
        ["q2", "q3"]
    );

    // 快照折叠（state_at → apply）与 derive 同一语义
    assert_eq!(s.state_at(s.len()).messages, msgs);
}

#[test]
fn sliding_compaction_without_summary_drops_head_only() {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(user(format!("q{i}")));
    }
    s.append(compacted(1, ""));
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 1, "无摘要：纯丢早期，只留尾部");
    assert_eq!(msgs[0], crate::Message::user("q2"));
}

#[test]
fn compaction_rewind_restores_full_history() {
    let mut s = Session::new();
    for i in 0..4 {
        s.append(user(format!("q{i}")));
    }
    let seq = s.append(compacted(2, "earlier notes"));
    s.append(user("after compact"));
    assert_eq!(s.derive_messages().len(), 4);

    // 撤销 = rewind_to 压缩事件（前缀游标，之后的轮次一并回退）
    s.rewind_to(seq);
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 4, "回到压缩前：四条原文全部可见");
    // fork 固化回退点后可继续追加（append 恢复推进游标）
    let mut forked = s.fork(seq);
    forked.append(user("next"));
    assert_eq!(forked.cursor(), forked.len());
    assert_eq!(forked.derive_messages().len(), 5);
}

#[test]
fn compaction_kept_over_length_is_noop() {
    let mut s = Session::new();
    s.append(user("q0"));
    s.append(compacted(10, "note"));
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 1, "kept 超过长度 = 空操作（不凭空造摘要）");
    assert_eq!(msgs[0], crate::Message::user("q0"));
}

#[test]
fn compaction_event_serializes_roundtrip() {
    // serde 往返：字段带 default，旧日志无此变体不受影响（向后兼容）
    let e = compacted(2, "note");
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["type"], "context_compacted");
    assert_eq!(v["kept_messages"], 2);
    let back: SessionEvent = serde_json::from_value(v).unwrap();
    assert_eq!(back, e);
}

#[test]
fn derive_from_offsets_to_the_tree_tail() {
    let mut s = Session::new();
    s.append(user("early"));
    s.append(SessionEvent::AssistantMessage {
        content: "early reply".into(),
        tool_calls: vec![],
    });
    s.append(user("tail question"));
    s.append(SessionEvent::AssistantMessage {
        content: "tail reply".into(),
        tool_calls: vec![],
    });

    // 尾巴投影 = [from..] 同一映射；全量投影不受影响
    let tail = s.derive_messages_from(2);
    assert_eq!(tail.len(), 2);
    assert!(matches!(&tail[0], crate::Message::User { content, .. } if content == "tail question"));
    assert_eq!(s.derive_messages().len(), 4);

    // from 越界 clamp 到长度 = 空表；from 超过游标（回退态）同样安全
    assert!(s.derive_messages_from(99).is_empty());
    s.rewind_to(2);
    assert_eq!(s.derive_messages_from(0).len(), 2, "游标裁剪语义不变");
    assert!(s.derive_messages_from(2).is_empty());
}

// ---- parts 穿透（批 D）----

#[test]
fn user_parts_carry_through_projection_fork_and_rewind() {
    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "看这张图".into(),
        parts: vec![img("a.png")],
    });
    s.append(SessionEvent::AssistantMessage {
        content: "reply".into(),
        tool_calls: vec![],
    });

    let msgs = s.derive_messages();
    let crate::Message::User { content, parts } = &msgs[0] else {
        panic!("expected user message");
    };
    assert_eq!(content, "看这张图");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0], img("a.png"));

    // fork（子代理前缀克隆）与 rewind/undo 重写都随消息自然携带
    let forked = s.fork(1);
    assert_eq!(forked.derive_messages()[0], msgs[0]);
    s.rewind_to(1);
    assert_eq!(
        s.derive_messages(),
        vec![crate::Message::user_with_parts(
            "看这张图",
            vec![img("a.png")]
        )]
    );
}

#[test]
fn old_jsonl_without_parts_reads_as_plain_text() {
    // 旧日志逐字节形态：UserMessage 无 parts 字段（serde default = 空）
    let line = r#"{"type":"user_message","content":"老消息"}"#;
    let e: SessionEvent = serde_json::from_str(line).unwrap();
    match &e {
        SessionEvent::UserMessage { content, parts } => {
            assert_eq!(content, "老消息");
            assert!(parts.is_empty());
        }
        other => panic!("unexpected event: {other:?}"),
    }
    // 新事件（带 parts）序列化多一个字段；无 parts 时不发明字段
    let v = serde_json::to_value(&e).unwrap();
    assert!(v.get("parts").is_none(), "空 parts 不落盘（字节前缀稳定）");
    let with_parts = SessionEvent::UserMessage {
        content: "带图".into(),
        parts: vec![img("b.png")],
    };
    let v = serde_json::to_value(&with_parts).unwrap();
    assert_eq!(v["parts"][0]["type"], "image");
}

#[test]
fn message_user_serde_parts_default_and_roundtrip() {
    let old = r#"{"role":"user","content":"老消息"}"#;
    let m: crate::Message = serde_json::from_str(old).unwrap();
    assert_eq!(m, crate::Message::user("老消息"));

    let with = crate::Message::user_with_parts("带图", vec![img("c.png")]);
    let v = serde_json::to_value(&with).unwrap();
    let back: crate::Message = serde_json::from_value(v).unwrap();
    assert_eq!(back, with);
}
