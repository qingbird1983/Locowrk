//! 投影面（自 `tests.rs` 批 F-23 拆出，用例本体逐字未改）：ToolCall 回填工具名与
//! 参数 / 尾巴投影 `derive_messages_from` 与全量同映射 / 整理段落总结投影成带标记的
//! User 消息。

use super::*;

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

// ---- 整理段落总结（Organize Summary Design v1 §3.2）----

#[test]
fn context_summary_projects_as_marked_user_message() {
    let mut s = Session::new();
    s.append(user("early"));
    s.append(assistant("early reply"));
    s.append(SessionEvent::ContextSummary {
        text: "此前完成了记忆树设计，决定加轮级总结。".into(),
    });
    s.append(user("tail question"));
    s.append(assistant("tail reply"));

    // 全量与尾巴投影都按日志真实位携带（断层缝合处 = 尾巴第一条）
    let msgs = s.derive_messages();
    let crate::Message::User { content, parts } = &msgs[2] else {
        panic!("expected user message at summary position");
    };
    assert!(parts.is_empty());
    assert!(
        content.starts_with("[context summary of earlier organized conversation]\n"),
        "content = {content}"
    );
    assert!(content.contains("轮级总结"));
    let tail = s.derive_messages_from(2);
    assert_eq!(tail.len(), 3);
    assert!(
        matches!(&tail[0], crate::Message::User { content, .. } if content.contains("context summary"))
    );
}
