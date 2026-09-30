//! 事件 serde 兼容（⑩③，exposure 文档 §3.4）：旧日志没有 plugin_id 字段，
//! 解析按 None 兜底；新日志带值原样往返。

use serde_json::json;

use crate::event::SessionEvent;
use plugin_api::{CallId, SideEffect};

#[test]
fn tool_call_without_plugin_id_parses_as_none() {
    let old = json!({
        "type": "tool_call",
        "call_id": "call-1",
        "tool": "echo",
        "args": { "text": "hi" },
        "plugin_version": "1.2.0",
        "side_effect": { "kind": "pure" }
    });
    let e: SessionEvent = serde_json::from_value(old).unwrap();
    match e {
        SessionEvent::ToolCall { plugin_id, .. } => assert_eq!(plugin_id, None),
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn tool_call_plugin_id_roundtrips() {
    let e = SessionEvent::ToolCall {
        call_id: CallId("c1".into()),
        tool: "fs_read".into(),
        args: json!({ "path": "/tmp/a" }),
        plugin_id: Some("tool-fs".into()),
        plugin_version: semver::Version::new(1, 2, 0),
        side_effect: SideEffect::Pure,
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["plugin_id"], json!("tool-fs"));
    let back: SessionEvent = serde_json::from_value(v).unwrap();
    assert_eq!(back, e);
}
