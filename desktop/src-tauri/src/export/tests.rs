//! export 渲染测试：分段聚合 / 工具与审批引用行 / 截断标注。

use super::*;
use crate::events::AgentEvent;
use serde_json::json;

fn ev(events: Vec<AgentEvent>) -> String {
    render_markdown("测试会话", "desktop-1", &events)
}

#[test]
fn sections_aggregate_and_order() {
    let md = ev(vec![
        AgentEvent::UserMessage {
            session_id: "desktop-1".into(),
            content: "帮我发邮件".into(),
        },
        AgentEvent::Chunk {
            session_id: "desktop-1".into(),
            delta: "好的，".into(),
        },
        AgentEvent::Chunk {
            session_id: "desktop-1".into(),
            delta: "我来发。".into(),
        },
        AgentEvent::UserMessage {
            session_id: "desktop-1".into(),
            content: "第二问".into(),
        },
    ]);
    assert!(md.starts_with("# 测试会话"));
    assert_eq!(md.matches("## 用户").count(), 2);
    assert_eq!(md.matches("## 助手").count(), 1, "连续 chunk 聚合为一段");
    assert!(md.contains("好的，我来发。"));
}

#[test]
fn tool_and_approval_lines() {
    let md = ev(vec![
        AgentEvent::ToolCall {
            session_id: "s".into(),
            call_id: "c1".into(),
            tool: "send_email".into(),
            args: json!({ "to": "a@b.c" }),
            side_effect: "irreversible".into(),
            plugin_version: "1.0.0".into(),
        },
        AgentEvent::ApprovalResolved {
            session_id: "s".into(),
            call_id: "c1".into(),
            tool: "send_email".into(),
            allowed: false,
            reason: Some("不发了".into()),
        },
        AgentEvent::ToolResult {
            session_id: "s".into(),
            call_id: "c1".into(),
            ok: false,
            output: json!(null),
            error: Some("用户拒绝".into()),
        },
    ]);
    assert!(md.contains("**工具 · send_email**"));
    assert!(md.contains("\"to\":\"a@b.c\"") || md.contains("\"to\": \"a@b.c\""));
    assert!(md.contains("> 审批记录：已拒绝 · send_email"));
    assert!(md.contains("> 工具结果 · 失败：用户拒绝"));
}

#[test]
fn long_output_is_clipped() {
    let long = "x".repeat(2000);
    let md = ev(vec![AgentEvent::ToolResult {
        session_id: "s".into(),
        call_id: "c".into(),
        ok: true,
        output: json!(long),
        error: None,
    }]);
    assert!(md.contains("…（截断）"));
    assert!(md.len() < 1200, "截断后不应携带全量输出");
}
