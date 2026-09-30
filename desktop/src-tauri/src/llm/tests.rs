//! llm 模块测试（全部无网络）：中立投影 ↔ genai 的映射、设置分发、
//! 缺 Key 的快速失败。genai Client 只构建不发请求。

use std::sync::Arc;

use kernel_agent::{LlmError, LlmRequest, LlmService};
use plugin_api::{CallId, SideEffect, ToolSpec};
use serde_json::json;
use session_store::{Message, ToolCallMsg};

use super::app_llm::AppLlm;
use super::genai_backend::{adapter_kind, to_genai_messages, to_genai_tools};
use crate::settings::SettingsStore;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("locowrk-llm-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn user_session() -> Vec<Message> {
    vec![
        Message::User {
            content: "查下天气".into(),
        },
        Message::Assistant {
            content: String::new(),
            tool_calls: vec![ToolCallMsg {
                id: CallId("call-9".into()),
                tool: "get_weather".into(),
                args: json!({ "city": "北京" }),
            }],
        },
        Message::ToolResult {
            call_id: CallId("call-9".into()),
            tool: "get_weather".into(),
            args: json!({ "city": "北京" }),
            output: json!({ "temp": "22C" }),
            error: None,
        },
    ]
}

/// user / assistant(带 tool_calls) / tool_result 三种消息的映射形状。
#[test]
fn messages_map_to_genai_shapes() {
    let msgs = to_genai_messages(&user_session());
    assert_eq!(msgs.len(), 3);

    let s = format!("{:?}", msgs[0]);
    assert!(s.contains("User") && s.contains("查下天气"));

    // assistant：正文为空时只有 ToolCall part，且带上 call_id / 函数名
    let s = format!("{:?}", msgs[1]);
    assert!(s.contains("ToolCall") && s.contains("call-9") && s.contains("get_weather"));

    // tool_result：走 ToolResponse part，内容是输出的 JSON 串
    let s = format!("{:?}", msgs[2]);
    assert!(s.contains("ToolResponse") && s.contains("call-9") && s.contains("22C"));
}

/// 工具失败也要闭合给模型：error 序列化成内容（模型可自行处理或转述）。
#[test]
fn tool_result_error_becomes_content() {
    let msgs = vec![Message::ToolResult {
        call_id: CallId("call-e".into()),
        tool: "shout".into(),
        args: json!({}),
        output: serde_json::Value::Null,
        error: Some("plugin crashed".into()),
    }];
    let s = format!("{:?}", to_genai_messages(&msgs));
    assert!(s.contains("ToolResponse") && s.contains("plugin crashed"));
}

#[test]
fn tools_map_to_openai_style() {
    let specs = vec![ToolSpec {
        name: "echo".into(),
        description: "回声工具".into(),
        schema: json!({ "type": "object", "properties": { "text": { "type": "string" } } }),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    }];
    let tools = to_genai_tools(&specs);
    assert_eq!(tools.len(), 1);
    let s = format!("{tools:?}");
    assert!(s.contains("echo") && s.contains("回声工具"));
}

#[test]
fn adapter_kind_follows_provider() {
    assert!(matches!(
        adapter_kind("openai"),
        genai::adapter::AdapterKind::OpenAI
    ));
    assert!(matches!(
        adapter_kind("anthropic"),
        genai::adapter::AdapterKind::Anthropic
    ));
    assert!(matches!(
        adapter_kind("ollama"),
        genai::adapter::AdapterKind::Ollama
    ));
}

/// model 未配置 → 演示替身（开箱即用，不碰网络）。
#[tokio::test]
async fn empty_model_falls_back_to_demo() {
    let root = temp_root("demo-fallback");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    let llm = AppLlm::new(Arc::clone(&store));

    let resp = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "hi".into(),
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(resp.content.contains("演示模型"));
    assert!(resp.tool_calls.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// 配好 model 且缺 Key → 真实路径的快速失败（提示去设置面板填）。
#[tokio::test]
async fn configured_model_without_key_fails_fast() {
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return; // 环境里有 Key 时该前置检查不触发，跳过
    }
    let root = temp_root("real-path");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    store
        .update(
            None,
            None,
            Some(crate::settings::LlmUpdate {
                provider: Some("openai".into()),
                model: Some("gpt-4o-mini".into()),
                base_url: None,
                api_key: None,
            }),
            None,
            None,
        )
        .unwrap();
    let llm = AppLlm::new(Arc::clone(&store));

    let err = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "hi".into(),
            }],
            ..Default::default()
        })
        .await
        .unwrap_err();
    let LlmError::Backend(msg) = err;
    assert!(msg.contains("OPENAI_API_KEY"), "got: {msg}");
    let _ = std::fs::remove_dir_all(&root);
}

/// 空 call_id 的响应（部分本地模型）按序补号，保证事件可关联。
#[test]
fn empty_call_ids_get_synthesized() {
    use genai::adapter::AdapterKind;
    use genai::chat::{ChatResponse, MessageContent, Usage};
    use genai::chat::{ContentPart, ToolCall as GenAiCall};
    use genai::ModelIden;

    let model = || ModelIden::new(AdapterKind::OpenAI, "test-model");
    let response = |calls: Vec<GenAiCall>, text: &str| ChatResponse {
        content: MessageContent::from_parts(
            [ContentPart::Text(text.to_string())]
                .into_iter()
                .chain(calls.into_iter().map(ContentPart::ToolCall))
                .collect::<Vec<_>>(),
        ),
        reasoning_content: None,
        model_iden: model(),
        provider_model_iden: model(),
        stop_reason: None,
        usage: Usage::default(),
        captured_raw_body: None,
        response_id: None,
    };

    let counter = std::sync::atomic::AtomicU64::new(1);

    // 有 call_id：原样保留
    let res = response(
        vec![GenAiCall {
            call_id: "abc".into(),
            fn_name: "echo".into(),
            fn_arguments: json!({ "text": "hi" }),
            thought_signatures: None,
        }],
        "",
    );
    let out = super::genai_backend::map_response(res, &counter);
    assert_eq!(out.tool_calls[0].id, CallId("abc".into()));
    assert_eq!(out.tool_calls[0].tool, "echo");
    assert_eq!(out.tool_calls[0].args, json!({ "text": "hi" }));

    // 空 call_id：补号从 call-1 开始（与 DemoLlm 同款约定）
    let res = response(
        vec![GenAiCall {
            call_id: String::new(),
            fn_name: "shout".into(),
            fn_arguments: json!({}),
            thought_signatures: None,
        }],
        "部分正文",
    );
    let out = super::genai_backend::map_response(res, &counter);
    assert_eq!(out.content, "部分正文");
    assert_eq!(out.tool_calls[0].id, CallId("call-1".into()));
}
