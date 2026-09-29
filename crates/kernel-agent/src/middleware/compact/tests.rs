//! ctx-compact 单测：假 LlmService + 事件总线直调，覆盖阈值两态、
//! fail-open、切点配对纪律、缺 LlmService 降级。

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::{Context, WaterfallDecision};
use plugin_api::{SideEffect, ToolSpec};
use serde_json::{Value, json};
use session_store::Message;

use crate::kernel::{LlmError, LlmRequest, LlmResponse, LlmService};
use crate::middleware::compact::{CompactConfig, CtxCompact};
use crate::waterfall::{RequestPayload, points};

struct FakeLlm {
    calls: Mutex<usize>,
    fail: bool,
}

impl FakeLlm {
    fn ok() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(0),
            fail: false,
        })
    }

    fn broken() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(0),
            fail: true,
        })
    }

    fn count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

#[async_trait]
impl LlmService for FakeLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        *self.calls.lock().unwrap() += 1;
        if self.fail {
            return Err(LlmError::Backend("summary backend down".into()));
        }
        Ok(LlmResponse {
            content: "SUMMARY-CONTENTS".into(),
            tool_calls: vec![],
        })
    }
}

fn catalog() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "fs.read".into(),
        description: "x".into(),
        schema: json!({ "type": "object" }),
        side_effect: SideEffect::Pure,
    }]
}

/// 交替的 User / Assistant / ToolResult 长消息流（把序列化体积撑过阈值）。
fn long_transcript(turns: usize) -> Vec<Message> {
    let filler = "x".repeat(400);
    let mut messages = Vec::new();
    for i in 0..turns {
        messages.push(Message::User {
            content: format!("turn {i}: {filler}"),
        });
        messages.push(Message::Assistant {
            content: format!("reply {i}: {filler}"),
            tool_calls: vec![],
        });
    }
    messages
}

fn request_payload(messages: Vec<Message>) -> Value {
    serde_json::to_value(RequestPayload {
        messages,
        model: Some("demo-model".into()),
        params: json!({ "temperature": 0.2 }),
        tools: catalog(),
    })
    .unwrap()
}

fn compact_over(ctx: &Arc<Context>, cfg: CompactConfig) -> CtxCompact {
    CtxCompact::new(Arc::clone(ctx), cfg)
}

#[tokio::test]
async fn under_threshold_is_passthrough() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::ok();
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(compact_over(
            &ctx,
            CompactConfig {
                max_tokens: 1_000_000,
                keep_recent: 4,
            },
        )),
    );

    let out = ctx
        .events
        .waterfall(points::REQUEST, request_payload(long_transcript(2)))
        .await;
    assert_eq!(out.decision, WaterfallDecision::Continue);
    assert_eq!(llm.count(), 0, "没超阈值不调摘要模型");
}

#[tokio::test]
async fn over_threshold_replaces_with_summary() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::ok();
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(compact_over(
            &ctx,
            CompactConfig {
                max_tokens: 100, // 人为压低阈值
                keep_recent: 4,
            },
        )),
    );

    let out = ctx
        .events
        .waterfall(points::REQUEST, request_payload(long_transcript(10)))
        .await;
    // Replace 是逐跳改写：终态决策 Continue，改写体现在 payload（cordis-core 语义）
    let req: RequestPayload = serde_json::from_value(out.payload).unwrap();
    assert_eq!(llm.count(), 1);
    let Message::User { content } = &req.messages[0] else {
        panic!("summary must be a User message");
    };
    assert!(content.contains("SUMMARY-CONTENTS"));
    assert!(req.messages.len() >= 5, "1 条摘要 + ≥keep_recent 条近期");
    // 摘要之外的原样字段
    assert_eq!(req.model.as_deref(), Some("demo-model"));
    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.params["temperature"], json!(0.2));
}

#[tokio::test]
async fn summary_failure_fails_open() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::broken();
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(compact_over(
            &ctx,
            CompactConfig {
                max_tokens: 100,
                keep_recent: 4,
            },
        )),
    );

    let payload = request_payload(long_transcript(6));
    let out = ctx.events.waterfall(points::REQUEST, payload).await;
    assert_eq!(out.decision, WaterfallDecision::Continue, "宁可不压不失败");
    assert_eq!(llm.count(), 1, "确实尝试过摘要");
}

#[tokio::test]
async fn cut_never_splits_tool_call_pairing() {
    // 切点纪律：压缩边界必须落在 User 之前。构造 naive 切点(=4)落在
    // Assistant 上的消息流：[U, A, TR, U, A, TR]，keep_recent=2。
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn LlmService>(FakeLlm::ok() as Arc<dyn LlmService>);
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(compact_over(
            &ctx,
            CompactConfig {
                max_tokens: 100,
                keep_recent: 2,
            },
        )),
    );

    let filler = "y".repeat(300);
    let call = crate::kernel::ToolCallReq {
        id: plugin_api::CallId("c1".into()),
        tool: "fs.read".into(),
        args: json!({}),
    };
    let messages = vec![
        Message::User {
            content: format!("q1 {filler}"),
        },
        Message::Assistant {
            content: format!("a1 {filler}"),
            tool_calls: vec![session_store::ToolCallMsg {
                id: call.id.clone(),
                tool: call.tool.clone(),
                args: call.args.clone(),
            }],
        },
        Message::ToolResult {
            call_id: call.id.clone(),
            tool: call.tool.clone(),
            args: call.args.clone(),
            output: json!({ "content": filler }),
            error: None,
        },
        Message::User {
            content: format!("q2 {filler}"),
        },
        Message::Assistant {
            content: format!("a2 {filler}"),
            tool_calls: vec![],
        },
        Message::ToolResult {
            call_id: call.id.clone(),
            tool: call.tool.clone(),
            args: call.args.clone(),
            output: json!({ "content": filler }),
            error: None,
        },
    ];
    let out = ctx
        .events
        .waterfall(points::REQUEST, request_payload(messages))
        .await;
    let req: RequestPayload = serde_json::from_value(out.payload).unwrap();
    // 摘要(1 条)之后的第一条必须仍是原始流里的 User（q2），不能是
    // Assistant/ToolResult 半截配对
    let Message::User { content } = &req.messages[1] else {
        panic!("cut must land right before a User message");
    };
    assert!(content.starts_with("q2"));
    assert_eq!(req.messages.len(), 4, "1 摘要 + q2/A/TR");
}

#[tokio::test]
async fn missing_llm_service_fails_open() {
    let ctx = Arc::new(Context::new()); // 故意不注册 LlmService
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(compact_over(
            &ctx,
            CompactConfig {
                max_tokens: 100,
                keep_recent: 4,
            },
        )),
    );
    let out = ctx
        .events
        .waterfall(points::REQUEST, request_payload(long_transcript(6)))
        .await;
    assert_eq!(out.decision, WaterfallDecision::Continue);
}

#[tokio::test]
async fn other_waterfall_points_ignored() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn LlmService>(FakeLlm::ok() as Arc<dyn LlmService>);
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(compact_over(
            &ctx,
            CompactConfig {
                max_tokens: 100,
                keep_recent: 4,
            },
        )),
    );
    let payload: Value = serde_json::to_value(RequestPayload {
        messages: long_transcript(6),
        model: None,
        params: json!({}),
        tools: vec![],
    })
    .unwrap();
    let out = ctx.events.waterfall(points::PRE_STEP, payload).await;
    assert_eq!(out.decision, WaterfallDecision::Continue);
}
