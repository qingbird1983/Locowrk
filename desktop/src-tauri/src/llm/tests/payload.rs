//! 线上协议装配（被测面 = `llm::wire`；簇名不叫 `wire` 是硬约束：子簇 `mod wire;` 会占住
//! `tests` 命名空间里的 `wire`，正文的 `super::wire::…` 就会指向测试簇自己而不是被测模块）。
//! 覆盖面：请求体形状 / 流式增量累积 / reasoning 分流 /
//! usage 字段映射 / 方言 thinking 字段与 `max_tokens` 预检钳制 / 自定义请求头装配。

use super::*;

// ---------- wire：请求装配 ----------

// ---------- wire：流式累积 ----------

// ---- 批 B：推理档位落 payload + max_tokens 预检钳制 ----

// ---- 自定义请求头装配（批 C §8.1）：最后写入者覆盖 + 双带鉴权现状 ----

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
        ..Default::default()
    };
    let p = build_payload("m", &req, true, Dialect::None, &caps, None).unwrap();
    assert_eq!(p["max_tokens"], 5_000);

    // 钉死 + 窗口充裕 = min(钉死, ctx − 估用 − 1000)；est 小 → 钉死赢
    let caps = WireCaps {
        max_output: Some(5_000),
        ctx_window: Some(100_000),
        ..Default::default()
    };
    let p = build_payload("m", &req, true, Dialect::None, &caps, None).unwrap();
    assert_eq!(p["max_tokens"], 5_000);

    // 窗口紧张 = 钳到余量；最低 1024（不钳到负数，留给溢出语义）
    let caps = WireCaps {
        max_output: Some(5_000),
        ctx_window: Some(2_000),
        ..Default::default()
    };
    let p = build_payload("m", &req, true, Dialect::None, &caps, None).unwrap();
    assert_eq!(p["max_tokens"], 1_024);
}

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
