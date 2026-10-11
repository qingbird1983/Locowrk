//! 剧本 LLM 路由（自 `tests.rs` 批 F-17 拆出，用例本体逐字未改）：demo_llm 按内容决定
//! 发工具 / 回纯文本 / 工具回执后汇总。

use super::*;

use crate::demo_llm::DemoLlm;
use kernel_agent::LlmRequest;
use serde_json::json;
use session_store::Message;

#[tokio::test]
async fn demo_llm_routes_by_content() {
    let llm = DemoLlm::new();

    let hit = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "请 echo 一下".into(),
                parts: Vec::new(),
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(hit.tool_calls.len(), 1);
    assert_eq!(hit.tool_calls[0].tool, "echo");

    let plain = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "你好".into(),
                parts: Vec::new(),
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(plain.tool_calls.is_empty());

    let after_tool = llm
        .complete(LlmRequest {
            messages: vec![Message::ToolResult {
                call_id: plugin_api::CallId("call-1".into()),
                tool: "echo".into(),
                args: json!({ "text": "hi" }),
                output: json!({ "echo": "hi" }),
                error: None,
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(after_tool.content.contains("工具返回"));
}
