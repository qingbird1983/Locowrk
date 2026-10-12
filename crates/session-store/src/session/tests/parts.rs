//! parts 穿透（批 D）与 serde 向后兼容（自 `tests.rs` 批 F-23 拆出，用例本体逐字
//! 未改）：附件随投影 / fork / rewind 自然携带 / 旧 jsonl 无 parts 读成纯文本 /
//! `Message::User` 的 parts 缺省与往返。

use super::*;

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
