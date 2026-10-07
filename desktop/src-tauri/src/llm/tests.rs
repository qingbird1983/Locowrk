//! llm 模块测试（全部无网络）：线上协议装配与解析（wire）、SSE 分帧
//! （sse）、错误分类与退避（retry）、设置分发与缺 Key 快速失败。

use std::sync::Arc;

use kernel_agent::{LlmError, LlmRequest, LlmService};
use plugin_api::{CallId, SideEffect, ToolSpec};
use serde_json::json;
use session_store::{Message, ToolCallMsg};

use super::app_llm::AppLlm;
use super::client::CompatLlm;
use super::retry::{self, Failure};
use super::sse::{SseEvent, SseParser};
use super::wire::{build_payload, parse_usage, StreamAcc};
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

// ---------- wire：请求装配 ----------

/// user / assistant(带 tool_calls) / tool_result 三种消息的线上形状；
/// system 注入为首位；流式才带 stream_options；不发明文派生限额。
#[test]
fn payload_assembles_openai_shapes() {
    let req = LlmRequest {
        messages: user_session(),
        system: Some("你是助手".into()),
        tools: vec![ToolSpec {
            name: "echo".into(),
            description: "回声工具".into(),
            schema: json!({ "type": "object" }),
            side_effect: SideEffect::Pure,
            exposure: Default::default(),
        }],
        params: json!({ "temperature": 0.3 }),
        ..Default::default()
    };
    let p = build_payload("test-model", &req, true);
    assert_eq!(p["model"], "test-model");
    assert_eq!(p["stream"], true);
    assert_eq!(p["stream_options"]["include_usage"], true);
    assert_eq!(p["temperature"], 0.3);
    assert!(p.get("max_tokens").is_none());

    let msgs = p["messages"].as_array().unwrap();
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "查下天气");
    // assistant：空正文 = content null + tool_calls 原样
    assert_eq!(msgs[2]["role"], "assistant");
    assert!(msgs[2]["content"].is_null());
    assert_eq!(msgs[2]["tool_calls"][0]["id"], "call-9");
    assert_eq!(msgs[2]["tool_calls"][0]["function"]["name"], "get_weather");
    // tool 结果：role tool + call_id 关联；工具失败以 JSON 内容回传
    assert_eq!(msgs[3]["role"], "tool");
    assert_eq!(msgs[3]["tool_call_id"], "call-9");
    assert!(msgs[3]["content"].as_str().unwrap().contains("22C"));

    let err_msgs = vec![Message::ToolResult {
        call_id: CallId("call-e".into()),
        tool: "shout".into(),
        args: json!({}),
        output: serde_json::Value::Null,
        error: Some("plugin crashed".into()),
    }];
    let p = build_payload(
        "m",
        &LlmRequest {
            messages: err_msgs,
            ..Default::default()
        },
        false,
    );
    assert!(p["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains("plugin crashed"));
    // 非流式：无 stream_options；无工具目录：无 tools 字段
    assert!(p.get("stream_options").is_none());
    assert!(p.get("tools").is_none());
}

// ---------- wire：流式累积 ----------

/// chunk 解析：正文 delta 外推、工具调用按 index 分片拼接、finish_reason
/// 收束、空 call_id 补号（与 DemoLlm 同款约定 call-1 起）。
#[test]
fn stream_acc_assembles_deltas_and_tool_calls() {
    let counter = std::sync::atomic::AtomicU64::new(1);
    let mut acc = StreamAcc::new();
    let chunk = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();

    let d = acc.apply(&chunk(r#"{"choices":[{"delta":{"content":"你"}}]}"#));
    assert_eq!(d, super::wire::Delta::Content("你".into()));
    acc.apply(&chunk(r#"{"choices":[{"delta":{"content":"好"}}]}"#));

    // 工具调用分片：id/name 与 arguments 分批到达
    acc.apply(&chunk(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"abc","function":{"name":"echo","arguments":"{\"te"}}]}}]}"#,
    ));
    acc.apply(&chunk(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"xt\":\"hi\"}"}}]}}]}"#,
    ));
    acc.apply(&chunk(
        r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
    ));

    let out = acc.finish(&counter);
    assert_eq!(out.content, "你好");
    assert_eq!(out.tool_calls.len(), 1);
    assert_eq!(out.tool_calls[0].id, CallId("abc".into()));
    assert_eq!(out.tool_calls[0].tool, "echo");
    assert_eq!(out.tool_calls[0].args, json!({ "text": "hi" }));
    assert_eq!(out.usage.as_ref().unwrap().input, 10);

    // 空 call_id：补号
    let mut acc = StreamAcc::new();
    acc.apply(&chunk(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"shout","arguments":""}}]}}]}"#,
    ));
    let out = acc.finish(&counter);
    assert_eq!(out.tool_calls[0].id, CallId("call-1".into()));
    assert_eq!(out.tool_calls[0].args, json!({}));
}

/// 推理 delta（reasoning_content / reasoning 变体）不进正文。
#[test]
fn reasoning_deltas_route_to_reasoning() {
    let mut acc = StreamAcc::new();
    let chunk = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
    assert_eq!(
        acc.apply(&chunk(
            r#"{"choices":[{"delta":{"reasoning_content":"思考"}}]}"#
        )),
        super::wire::Delta::Reasoning("思考".into())
    );
    assert_eq!(
        acc.apply(&chunk(r#"{"choices":[{"delta":{"reasoning":"续"}}]}"#)),
        super::wire::Delta::Reasoning("续".into())
    );
    let counter = std::sync::atomic::AtomicU64::new(1);
    assert_eq!(acc.finish(&counter).content, "", "推理不占正文");
}

/// 用量映射：OpenAI cached_tokens / DeepSeek prompt_cache_hit_tokens /
/// Anthropic 形 cache_creation_tokens；全 0 = None（宁缺勿假）。
#[test]
fn usage_maps_cache_and_reasoning_fields() {
    let v = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
    let u = parse_usage(&v(
        r#"{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,
            "prompt_tokens_details":{"cached_tokens":80},"completion_tokens_details":{"reasoning_tokens":12}}"#,
    ))
    .unwrap();
    assert_eq!(
        (u.input, u.output, u.cache_read, u.reasoning, u.total),
        (100, 20, 80, 12, 120)
    );

    let u = parse_usage(&v(
        r#"{"prompt_tokens":100,"completion_tokens":20,"prompt_cache_hit_tokens":90,"prompt_cache_miss_tokens":10}"#,
    ))
    .unwrap();
    assert_eq!(u.cache_read, 90);

    assert_eq!(
        parse_usage(&v(r#"{"prompt_tokens":0,"completion_tokens":0}"#)),
        None
    );
}

// ---------- sse：分帧 ----------

/// 按行缓冲跨网络分块拼帧；注释/keep-alive 跳过；[DONE] 哨兵；缺尾空行
/// 由 finish 冲刷。
#[test]
fn sse_parser_buffers_lines_across_chunks() {
    let mut p = SseParser::new();
    let mut out = Vec::new();
    p.feed("data: {\"a\"", &mut out);
    p.feed(":1}\n\n: keep-alive\ndata: [DON", &mut out);
    assert_eq!(out.len(), 1, "第一条跨块凑齐");
    assert_eq!(out[0], SseEvent::Data("{\"a\":1}".into()));
    p.feed("E]\n", &mut out); // [DONE] 行无尾空行
    assert_eq!(out.len(), 2);
    assert_eq!(out[1], SseEvent::Done);
    p.finish(&mut out); // 残留（无）——幂等
    assert_eq!(out.len(), 2);
}

// ---------- retry：分类与退避 ----------

/// 分类表：鉴权 fatal / 429 限流 / 5xx·408 瞬时 / 上下文超限 fatal /
/// 普通 4xx fatal（带报文片段）。
#[test]
fn classify_follows_status_and_body() {
    assert!(matches!(retry::classify_status(401, ""), Failure::Fatal(_)));
    assert!(matches!(retry::classify_status(403, ""), Failure::Fatal(_)));
    assert!(matches!(
        retry::classify_status(429, "rate limited"),
        Failure::RateLimit {
            retry_after_ms: None
        }
    ));
    assert!(matches!(
        retry::classify_status(502, ""),
        Failure::Transient { .. }
    ));
    assert!(matches!(
        retry::classify_status(408, ""),
        Failure::Transient { .. }
    ));
    // 400 + 上下文超限语义 → fatal（重试不会变小）
    assert!(matches!(
        retry::classify_status(400, "maximum context length exceeded"),
        Failure::Fatal(_)
    ));
    assert!(matches!(
        retry::classify_status(413, "too many tokens"),
        Failure::Fatal(_)
    ));
    // 普通 400 = 请求错误，如实透传
    let f = retry::classify_status(400, "invalid field 'foo'");
    assert!(matches!(f, Failure::Fatal(_)));
}

/// 退避曲线：限流 2s×2 指数 + 抖动上界、封顶 30s、Retry-After 优先；
/// 瞬时 1/2/4/8s 后维持 8s。
#[test]
fn backoff_curves_match_policy() {
    // 限流：attempt=1 → 2s..2.5s；attempt=3 → 8s..10s
    let d = retry::rate_limit_delay_ms(1, None, 1.0);
    assert_eq!(d, 2_500);
    let d = retry::rate_limit_delay_ms(1, None, 0.0);
    assert_eq!(d, 2_000);
    let d = retry::rate_limit_delay_ms(3, None, 0.0);
    assert_eq!(d, 8_000);
    assert_eq!(
        retry::rate_limit_delay_ms(20, None, 1.0),
        retry::RATE_LIMIT_MAX_MS
    );
    // 服务器声明优先（封顶 30s）
    assert_eq!(retry::rate_limit_delay_ms(1, Some(5_000), 0.0), 5_000);
    assert_eq!(
        retry::rate_limit_delay_ms(1, Some(999_999), 0.0),
        retry::RATE_LIMIT_MAX_MS
    );
    // 瞬时：1/2/4/8/8...
    assert_eq!(retry::transient_delay_ms(1, None), 1_000);
    assert_eq!(retry::transient_delay_ms(2, None), 2_000);
    assert_eq!(retry::transient_delay_ms(3, None), 4_000);
    assert_eq!(retry::transient_delay_ms(4, None), 8_000);
    assert_eq!(retry::transient_delay_ms(9, None), 8_000);
    assert_eq!(retry::transient_delay_ms(2, Some(3_000)), 3_000);
}

// ---------- 端点与设置分发 ----------

/// 端点归一：缺版本段补 /v1；ollama 落官方兼容端点 /v1；智谱 /v4、方舟
/// /v3 原样保留（LLM.md 覆盖表）。
#[test]
fn endpoint_normalizes_join_version() {
    let cfg = |provider: &str, base: Option<&str>| crate::settings::LlmSettings {
        provider: provider.into(),
        model: "m".into(),
        base_url: base.map(Into::into),
        api_key: None,
        window_tokens: None,
    };
    assert_eq!(
        CompatLlm::endpoint(&cfg("openai", Some("https://api.deepseek.com"))),
        "https://api.deepseek.com/v1/chat/completions"
    );
    assert_eq!(
        CompatLlm::endpoint(&cfg("openai", Some("https://open.bigmodel.cn/api/paas/v4"))),
        "https://open.bigmodel.cn/api/paas/v4/chat/completions"
    );
    assert_eq!(
        CompatLlm::endpoint(&cfg("anthropic", None)),
        "https://api.anthropic.com/v1/chat/completions"
    );
    assert_eq!(
        CompatLlm::endpoint(&cfg("ollama", Some("http://localhost:11434"))),
        "http://localhost:11434/v1/chat/completions"
    );
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
            None,
            Some(crate::settings::LlmUpdate {
                provider: Some("openai".into()),
                model: Some("gpt-4o-mini".into()),
                base_url: None,
                api_key: None,
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
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

/// v2.58 §3.30 会话级模型覆盖解析：覆盖命中 = 整档换路由（kind/端点/Key
/// 随档位）+ 模型名替换；未命中会话 / 无注入 = 设置缺省；决策环
/// `req.model`（waterfall 契约）优先级最高。
#[test]
fn effective_cfg_resolves_session_override() {
    use crate::model_override::ModelOverrideStore;
    use kernel_agent::LlmRequest;

    let root = temp_root("override-cfg");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    // 活动档 default（openai/m1）+ 第二档 p2（anthropic/m2 自带端点）
    store
        .update_llm_config(|cfg| {
            cfg.providers.push(crate::settings::ProviderProfile {
                id: "default".into(),
                label: "default".into(),
                kind: "openai".into(),
                model: "m1".into(),
                enabled: true,
                ..Default::default()
            });
            cfg.providers.push(crate::settings::ProviderProfile {
                id: "p2".into(),
                label: "P2".into(),
                kind: "anthropic".into(),
                base_url: Some("https://relay.example.com".into()),
                api_key: Some("sk-2".into()),
                model: "m2".into(),
                window_tokens: Some(200_000),
                enabled: true,
                models: Vec::new(),
            });
            cfg.active = "default".into();
            Ok(())
        })
        .unwrap();

    let dir = temp_root("override-sidecar");
    let overrides = Arc::new(ModelOverrideStore::new(dir.clone()));
    let llm = AppLlm::with_overrides(Arc::clone(&store), Arc::clone(&overrides));
    let req = |sid: Option<&str>, model: Option<&str>| LlmRequest {
        session_id: sid.map(Into::into),
        model: model.map(Into::into),
        ..Default::default()
    };

    // 未覆盖会话 = 设置缺省（活动档）
    let cfg = llm.effective_cfg(&req(Some("s1"), None));
    assert_eq!(
        (cfg.provider.as_str(), cfg.model.as_str()),
        ("openai", "m1")
    );

    // 覆盖命中 = 换档路由 + 模型名
    overrides.set("s1", Some(("p2", "m2-x"))).unwrap();
    let cfg = llm.effective_cfg(&req(Some("s1"), None));
    assert_eq!(cfg.provider, "anthropic");
    assert_eq!(cfg.model, "m2-x");
    assert_eq!(cfg.base_url.as_deref(), Some("https://relay.example.com"));
    assert_eq!(cfg.api_key.as_deref(), Some("sk-2"));
    assert_eq!(cfg.window_tokens, Some(200_000));

    // 决策环 model 改写压过会话覆盖
    let cfg = llm.effective_cfg(&req(Some("s1"), Some("m-decision")));
    assert_eq!(cfg.model, "m-decision");

    // 无注入存储 = 永远缺省（测试路径）
    let plain = AppLlm::new(Arc::clone(&store));
    let cfg = plain.effective_cfg(&req(Some("s1"), None));
    assert_eq!(cfg.model, "m1");
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&dir).ok();
}
