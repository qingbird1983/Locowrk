//! llm 模块测试（全部无网络）：线上协议装配与解析（wire）、SSE 分帧
//! （sse）、错误分类与退避（retry）、设置分发与缺 Key 快速失败。

use std::sync::Arc;

use kernel_agent::{LlmError, LlmRequest, LlmService};
use plugin_api::{CallId, SideEffect, ToolSpec};
use serde_json::json;
use session_store::{Message, ToolCallMsg};

use super::app_llm::AppLlm;
use super::client::CompatLlm;
use super::dialect::Dialect;
use super::retry::{self, Failure};
use super::sse::{SseEvent, SseParser};
use super::wire::{build_payload, parse_usage, StreamAcc, WireCaps};
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
            parts: Vec::new(),
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
    let p = build_payload(
        "test-model",
        &req,
        true,
        Dialect::None,
        &WireCaps::default(),
        None,
    )
    .unwrap();
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
        Dialect::None,
        &WireCaps::default(),
        None,
    )
    .unwrap();
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
        ..Default::default()
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
                parts: Vec::new(),
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
                None,
)
        .unwrap();
    let llm = AppLlm::new(Arc::clone(&store));

    let err = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "hi".into(),
                parts: Vec::new(),
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
                ..Default::default()
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

// ---- 批 B：推理档位落 payload + max_tokens 预检钳制 ----

#[test]
fn payload_carries_dialect_thinking_fields() {
    let req = LlmRequest {
        params: json!({ "thinking_level": "low" }),
        ..Default::default()
    };
    // deepseek_thinking：thinking enabled + reasoning_effort 双字段
    let p = build_payload(
        "m",
        &req,
        true,
        Dialect::DeepseekThinking,
        &WireCaps::default(),
        None,
    )
    .unwrap();
    assert_eq!(p["thinking"], json!({ "type": "enabled" }));
    assert_eq!(p["reasoning_effort"], "low");

    // 无方言（None）：不发任何档位字段（宁缺勿假）
    let p = build_payload("m", &req, true, Dialect::None, &WireCaps::default(), None).unwrap();
    assert!(p.get("thinking").is_none());
    assert!(p.get("reasoning_effort").is_none());

    // 档位在方言表没有写法：字段缺席
    let req_bad = LlmRequest {
        params: json!({ "thinking_level": "enabled" }),
        ..Default::default()
    };
    let p = build_payload(
        "m",
        &req_bad,
        true,
        Dialect::DeepseekThinking,
        &WireCaps::default(),
        None,
    )
    .unwrap();
    assert!(p.get("thinking").is_none());
    assert!(p.get("reasoning_effort").is_none());

    // 无档位：不发（模型走自家缺省）
    let req_none = LlmRequest::default();
    let p = build_payload(
        "m",
        &req_none,
        true,
        Dialect::DeepseekThinking,
        &WireCaps::default(),
        None,
    )
    .unwrap();
    assert!(p.get("thinking").is_none());
}

#[test]
fn max_tokens_only_when_pinned_and_preflight_clamped() {
    let req = LlmRequest {
        messages: vec![Message::User {
            content: "hi".into(),
            parts: Vec::new(),
            }],
        ..Default::default()
    };
    // 未钉死：不发（v2.75 纪律不变）
    let p = build_payload("m", &req, true, Dialect::None, &WireCaps::default(), None).unwrap();
    assert!(p.get("max_tokens").is_none());

    // 钉死 + 无窗口 = 原值发送
    let caps = WireCaps {
        max_output: Some(5_000),
        ctx_window: None,
    };
    let p = build_payload("m", &req, true, Dialect::None, &caps, None).unwrap();
    assert_eq!(p["max_tokens"], 5_000);

    // 钉死 + 窗口充裕 = min(钉死, ctx − 估用 − 1000)；est 小 → 钉死赢
    let caps = WireCaps {
        max_output: Some(5_000),
        ctx_window: Some(100_000),
    };
    let p = build_payload("m", &req, true, Dialect::None, &caps, None).unwrap();
    assert_eq!(p["max_tokens"], 5_000);

    // 窗口紧张 = 钳到余量；最低 1024（不钳到负数，留给溢出语义）
    let caps = WireCaps {
        max_output: Some(5_000),
        ctx_window: Some(2_000),
    };
    let p = build_payload("m", &req, true, Dialect::None, &caps, None).unwrap();
    assert_eq!(p["max_tokens"], 1_024);
}

// ---- 自定义请求头装配（批 C §8.1）：最后写入者覆盖 + 双带鉴权现状 ----

#[test]
fn build_headers_last_writer_wins() {
    let mut cfg = crate::settings::LlmSettings {
        provider: "anthropic".into(),
        model: "m".into(),
        ..Default::default()
    };
    cfg.headers
        .insert("anthropic-version".into(), "2040-01-01".into());
    cfg.headers
        .insert("User-Agent".into(), "relay-gate/1".into());
    let map = CompatLlm::build_headers(&cfg, "sk-test");
    // 用户头覆盖默认头（HeaderMap insert 替换语义）
    assert_eq!(map.get("anthropic-version").unwrap(), "2040-01-01");
    assert_eq!(map.get("User-Agent").unwrap(), "relay-gate/1");
    // anthropic 双带鉴权（v3.0 现状不变）
    assert_eq!(map.get("x-api-key").unwrap(), "sk-test");
    assert_eq!(map.get("authorization").unwrap(), "Bearer sk-test");
    // openai 只有 Bearer；ollama 无鉴权
    let m2 = CompatLlm::build_headers(
        &crate::settings::LlmSettings {
            provider: "openai".into(),
            ..Default::default()
        },
        "k2",
    );
    assert_eq!(m2.get("authorization").unwrap(), "Bearer k2");
    assert!(m2.get("x-api-key").is_none());
    // ollama 走同一 `_ =>` 臂：空 Key 与旧 bearer_auth("") 行为逐字节一致
    let m3 = CompatLlm::build_headers(
        &crate::settings::LlmSettings {
            provider: "ollama".into(),
            ..Default::default()
        },
        "",
    );
    assert_eq!(m3.get("authorization").unwrap(), "Bearer ");
}

// ---- 批 E：工具结果图片投影（§8.1，D4 按推荐落档） ----

fn asset_output() -> serde_json::Value {
    json!({
        "text": "screenshot 1920x1080 captured",
        "asset": { "id": "abc123", "mime": "image/png", "name": "shot.png" }
    })
}

fn tool_msg(output: serde_json::Value) -> Message {
    Message::ToolResult {
        call_id: CallId("call-1".into()),
        tool: "screenshot".into(),
        args: json!({}),
        output,
        error: None,
    }
}

#[test]
fn tool_image_projects_into_tail_user_message() {
    // agent 循环常态：工具结果在消息列尾部（其后无 user）→ 合成一条
    // 只带图片的 user 消息追加，tool 消息剥引用 + note
    let out = super::wire::project_tool_assets(
        vec![
            Message::User {
                content: "截个屏".into(),
                parts: Vec::new(),
            },
            tool_msg(asset_output()),
        ],
        Some(true),
    );
    assert_eq!(out.len(), 3);
    match &out[1] {
        Message::ToolResult { output, .. } => {
            assert!(output.get("asset").is_none(), "引用号已剥");
            assert_eq!(
                output["image_note"],
                "[图片已附于本工具结果之后: shot.png]"
            );
            assert_eq!(output["text"], "screenshot 1920x1080 captured");
        }
        other => panic!("expected tool result, got {other:?}"),
    }
    match &out[2] {
        Message::User { content, parts } => {
            assert_eq!(content, "[工具产生的图片]");
            assert_eq!(parts.len(), 1);
        }
        other => panic!("expected synthetic user, got {other:?}"),
    }
}

#[test]
fn historical_tool_image_not_resent_but_note_stable() {
    // 历史轮：工具结果之后已有 user 消息 → 图片不重发（token 不膨胀），
    // 但 note 文案与尾部轮逐字节一致（M1 前缀缓存纪律）
    let tail = super::wire::project_tool_assets(vec![tool_msg(asset_output())], Some(true));
    let tail_note = match &tail[0] {
        Message::ToolResult { output, .. } => output["image_note"].clone(),
        other => panic!("expected tool result, got {other:?}"),
    };
    let grown = super::wire::project_tool_assets(
        vec![
            tool_msg(asset_output()),
            Message::Assistant {
                content: "屏幕显示…".into(),
                tool_calls: Vec::new(),
            },
            Message::User {
                content: "再来一张".into(),
                parts: Vec::new(),
            },
        ],
        Some(true),
    );
    assert_eq!(grown.len(), 3, "历史轮不合成 user 消息");
    match &grown[0] {
        Message::ToolResult { output, .. } => {
            assert_eq!(output["image_note"], tail_note, "note 跨请求字节稳定");
            assert!(output.get("asset").is_none());
        }
        other => panic!("expected tool result, got {other:?}"),
    }
    // 尾部合成消息只出现一次且在末尾
    let tail2 = super::wire::project_tool_assets(
        vec![tool_msg(asset_output()), Message::User {
            content: "hi".into(),
            parts: Vec::new(),
        }],
        Some(true),
    );
    assert_eq!(tail2.len(), 2);
    assert!(matches!(tail2[1], Message::User { ref parts, .. } if parts.is_empty()));
}

#[test]
fn unsupported_model_keeps_asset_ref_with_note() {
    let out = super::wire::project_tool_assets(vec![tool_msg(asset_output())], Some(false));
    assert_eq!(out.len(), 1, "不支持 = 不投影不合成");
    match &out[0] {
        Message::ToolResult { output, .. } => {
            assert_eq!(output["asset"]["id"], "abc123", "引用号留原地");
            assert_eq!(
                output["image_note"],
                "[图片未随请求发送：当前模型不支持图片输入]"
            );
        }
        other => panic!("expected tool result, got {other:?}"),
    }
}

#[test]
fn non_image_assets_and_plain_output_untouched() {
    // PDF 引用不走图片投影（原地保留，零改写）
    let pdf_out = json!({
        "text": "readme.pdf stored",
        "asset": { "id": "pdf1", "mime": "application/pdf", "name": "readme.pdf" }
    });
    let out = super::wire::project_tool_assets(vec![tool_msg(pdf_out)], Some(true));
    match &out[0] {
        Message::ToolResult { output, .. } => assert_eq!(output["asset"]["id"], "pdf1"),
        other => panic!("expected tool result, got {other:?}"),
    }
    // 纯文本工具结果与多资产（assets 数组）保序
    let multi = json!({
        "text": "3 pages",
        "assets": [
            { "id": "p1", "mime": "image/png", "name": "page-1.png" },
            { "id": "p2", "mime": "image/png", "name": "page-2.png" }
        ]
    });
    let out = super::wire::project_tool_assets(vec![tool_msg(multi)], None);
    assert_eq!(out.len(), 2, "None = 放行投影");
    match &out[1] {
        Message::User { parts, .. } => {
            let ids: Vec<&str> = parts
                .iter()
                .map(|p| match p {
                    session_store::ContentPart::Image { asset } => asset.id.as_str(),
                    _ => "",
                })
                .collect();
            assert_eq!(ids, vec!["p1", "p2"], "多资产保序（页序不翻面）");
        }
        other => panic!("expected synthetic user, got {other:?}"),
    }
    // 跨结果也保序：A[p1,p2] 在前、B[b1] 在后 → 合成消息 [p1,p2,b1]
    let two = vec![
        Message::User { content: "hi".into(), parts: Vec::new() },
        tool_msg(json!({"assets": [
            { "id": "a1", "mime": "image/png", "name": "a1.png" },
            { "id": "a2", "mime": "image/png", "name": "a2.png" }
        ]})),
        tool_msg(json!({ "asset": { "id": "b1", "mime": "image/png", "name": "b1.png" }})),
    ];
    let out = super::wire::project_tool_assets(two, None);
    match out.last() {
        Some(Message::User { parts, .. }) => {
            let ids: Vec<&str> = parts
                .iter()
                .map(|p| match p {
                    session_store::ContentPart::Image { asset } => asset.id.as_str(),
                    _ => "",
                })
                .collect();
            assert_eq!(ids, vec!["a1", "a2", "b1"], "跨结果保序");
        }
        other => panic!("expected synthetic user, got {other:?}"),
    }
}
