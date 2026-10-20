//! 投影面（自 `tests.rs` 批 F-23 拆出，用例本体逐字未改）：ToolCall 回填工具名与
//! 参数 / 尾巴投影 `derive_messages_from` 与全量同映射。
//!
//! S4 刀③c：`ContextSummary`（整理段落总结）随记忆整线退役，原第三条用例
//! 一并删除——不是为过闸删测试，变体本身不在协议面上了。

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
