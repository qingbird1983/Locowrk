//! ctx-compact 单测（⑨ 配置驱动重写）：三策略、目标 50%、降级链
//! （摘要失败 / 缺 LlmService → 滑动窗口）、切点配对纪律、幂等守卫、
//! 配置热更新、建议值表。

use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use cordis_core::{Context, WaterfallDecision};
use plugin_api::{SideEffect, ToolSpec};
use serde_json::{Value, json};
use session_store::Message;

use crate::kernel::{LlmError, LlmRequest, LlmResponse, LlmService};
use crate::middleware::compact::{
    CompactConfig, CompactStrategy, CompactionOutcome, CtxCompact, suggested_threshold,
};
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
            usage: None,
        })
    }
}

fn catalog() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "fs_read".into(),
        description: "x".into(),
        schema: json!({ "type": "object" }),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    }]
}

/// 交替的 User / Assistant 长消息流（每条约 100 token，把总量撑过阈值）。
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

fn config(threshold: u64, strategy: CompactStrategy) -> Arc<RwLock<CompactConfig>> {
    Arc::new(RwLock::new(CompactConfig {
        threshold_tokens: threshold,
        strategy,
        ..CompactConfig::default()
    }))
}

fn request_payload(messages: Vec<Message>) -> Value {
    serde_json::to_value(RequestPayload {
        messages,
        model: Some("demo-model".into()),
        params: json!({ "temperature": 0.2 }),
        tools: catalog(),
        compacted: None,
        system: None,
        session_id: None,
        tree_directory: None,
    })
    .unwrap()
}

/// 挂上监听器并跑一轮决策环，返回出环 payload。
async fn run(
    ctx: &Arc<Context>,
    cfg: Arc<RwLock<CompactConfig>>,
    llm: Option<Arc<dyn LlmService>>,
    messages: Vec<Message>,
) -> Value {
    if let Some(l) = llm {
        ctx.provide_dyn::<dyn LlmService>(l);
    }
    ctx.events
        .on_waterfall(points::REQUEST, Arc::new(CtxCompact::new(ctx.clone(), cfg)));
    ctx.events
        .waterfall(points::REQUEST, request_payload(messages))
        .await
        .payload
}

#[tokio::test]
async fn under_threshold_is_passthrough() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::ok();
    let payload = run(
        &ctx,
        config(1_000_000, CompactStrategy::Layered),
        Some(llm.clone()),
        long_transcript(2),
    )
    .await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    assert!(req.compacted.is_none());
    assert_eq!(llm.count(), 0, "没超阈值不调摘要模型");
}

#[tokio::test]
async fn layered_over_threshold_replaces_with_summary() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::ok();
    let payload = run(
        &ctx,
        config(400, CompactStrategy::Layered),
        Some(llm.clone()),
        long_transcript(10),
    )
    .await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    assert_eq!(llm.count(), 1);
    let Some(c) = &req.compacted else {
        panic!("压缩结果必须写回 payload");
    };
    assert_eq!(c.strategy, "layered");
    assert!(c.summary.contains("SUMMARY-CONTENTS"));
    assert!(c.from_tokens > c.to_tokens, "压缩必须减小体积");
    assert!(c.to_tokens < 400, "压到阈值一半附近（§3.4），不贴线");
    let Message::User { content } = &req.messages[0] else {
        panic!("摘要必须是 User 消息");
    };
    assert!(content.contains("SUMMARY-CONTENTS"));
    assert_eq!(
        req.messages.len(),
        c.kept_messages + 1,
        "1 条摘要 + 预算内近期原文"
    );
    assert!(c.kept_messages >= 2, "预算（阈值一半）至少容得下最近一轮");
    // 摘要之外的原样字段
    assert_eq!(req.model.as_deref(), Some("demo-model"));
    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.params["temperature"], json!(0.2));
}

#[tokio::test]
async fn sliding_window_never_calls_llm() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::ok();
    let payload = run(
        &ctx,
        config(400, CompactStrategy::SlidingWindow),
        Some(llm.clone()),
        long_transcript(10),
    )
    .await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    assert_eq!(llm.count(), 0, "滑动窗口纯本地，不调模型");
    let c = req.compacted.expect("滑动窗口也写回压缩结果");
    assert_eq!(c.strategy, "sliding_window");
    assert!(c.summary.is_empty());
    // 没有摘要消息：第一条直接是保留的近期原文
    assert!(matches!(req.messages[0], Message::User { .. }));
}

#[tokio::test]
async fn summary_only_cuts_at_last_user_boundary() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::ok();
    // 真实请求流以 User 收尾（当前轮还没有 assistant 回复）：
    // [U0, A0, ..., U8, A8, U9] → 仅摘要 = 只保留 U9，其余进摘要
    let filler = "x".repeat(400);
    let mut messages = long_transcript(9);
    messages.push(Message::User {
        content: format!("turn 9: {filler}"),
    });
    let payload = run(
        &ctx,
        config(400, CompactStrategy::SummaryOnly),
        Some(llm.clone()),
        messages,
    )
    .await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    assert_eq!(llm.count(), 1);
    let c = req.compacted.expect("压缩结果写回");
    assert_eq!(c.strategy, "summary_only");
    assert_eq!(c.kept_messages, 1, "只保留当前轮用户消息");
    assert_eq!(req.messages.len(), 2, "摘要 + 当前轮");
    let Message::User { content } = &req.messages[1] else {
        panic!("保留段以当前轮 User 开头");
    };
    assert!(content.starts_with("turn 9"));
}

#[tokio::test]
async fn summary_failure_degrades_to_sliding() {
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::broken();
    let payload = run(
        &ctx,
        config(400, CompactStrategy::Layered),
        Some(llm.clone()),
        long_transcript(10),
    )
    .await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    assert_eq!(llm.count(), 1, "确实尝试过摘要");
    // §3.5：摘要失败降级滑动窗口——本地裁剪仍然保窗口，不放过溢出
    let c = req.compacted.expect("降级后仍是本地压缩");
    assert_eq!(c.strategy, "sliding_window");
    assert!(c.summary.is_empty());
    assert!(c.from_tokens > c.to_tokens);
}

#[tokio::test]
async fn missing_llm_service_degrades_to_sliding() {
    let ctx = Arc::new(Context::new()); // 故意不注册 LlmService
    let payload = run(
        &ctx,
        config(400, CompactStrategy::Layered),
        None,
        long_transcript(10),
    )
    .await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    let c = req
        .compacted
        .expect("缺 LlmService 降级滑动窗口，不放任溢出");
    assert_eq!(c.strategy, "sliding_window");
}

#[tokio::test]
async fn cut_never_splits_tool_call_pairing() {
    // 切点纪律：压缩边界必须落在 User 之前。构造 [U, A, TR, U, A, TR]，
    // 预算只够留下尾部两条（A, TR）→ 边界必须回退到第二条 User 之前。
    let ctx = Arc::new(Context::new());
    let filler = "y".repeat(300);
    let call = crate::kernel::ToolCallReq {
        id: plugin_api::CallId("c1".into()),
        tool: "fs_read".into(),
        args: json!({}),
        session_id: None,
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
    let payload = run(
        &ctx,
        config(400, CompactStrategy::SlidingWindow),
        None,
        messages,
    )
    .await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    // 滑动窗口无摘要：第一条保留消息必须仍是原始流里的 User（q2），
    // 不能是 Assistant/ToolResult 半截配对
    let Message::User { content } = &req.messages[0] else {
        panic!("cut must land right before a User message");
    };
    assert!(content.starts_with("q2"));
    assert_eq!(req.messages.len(), 3, "q2/A/TR 三条留全");
}

#[tokio::test]
async fn already_compacted_payload_is_untouched() {
    // 幂等守卫：payload 已带压缩结果（重试轮）就不再压
    let ctx = Arc::new(Context::new());
    let llm = FakeLlm::ok();
    ctx.provide_dyn::<dyn LlmService>(llm.clone() as Arc<dyn LlmService>);
    let cfg = config(1, CompactStrategy::Layered); // 阈值压到最低必触发
    ctx.events
        .on_waterfall(points::REQUEST, Arc::new(CtxCompact::new(ctx.clone(), cfg)));
    let mut payload = request_payload(long_transcript(10));
    payload["compacted"] = serde_json::to_value(CompactionOutcome {
        from_tokens: 999,
        to_tokens: 99,
        strategy: "layered".into(),
        summary: "earlier".into(),
        kept_messages: 2,
    })
    .unwrap();
    let out = ctx.events.waterfall(points::REQUEST, payload).await;
    assert_eq!(out.decision, WaterfallDecision::Continue);
    assert_eq!(llm.count(), 0);
}

#[tokio::test]
async fn config_changes_take_effect_without_rebuild() {
    // 配置驱动：组装点换入新配置，下一请求即生效（⑨ 设置保存即用）
    let ctx = Arc::new(Context::new());
    let cfg = config(1_000_000, CompactStrategy::Layered);
    let llm = FakeLlm::ok();
    let payload = run(&ctx, cfg.clone(), Some(llm.clone()), long_transcript(6)).await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    assert!(req.compacted.is_none(), "高阈值下不压");
    *cfg.write().unwrap() = CompactConfig {
        threshold_tokens: 400,
        strategy: CompactStrategy::Layered,
        ..CompactConfig::default()
    };
    let payload = run(&ctx, cfg, Some(llm.clone()), long_transcript(6)).await;
    let req: RequestPayload = serde_json::from_value(payload).unwrap();
    assert!(req.compacted.is_some(), "换入低阈值后即压");
}

#[test]
fn suggested_threshold_follows_window_table() {
    assert_eq!(suggested_threshold(8_000), 30, "本地小模型");
    assert_eq!(suggested_threshold(16_000), 30);
    assert_eq!(suggested_threshold(32_000), 35, "中等窗口");
    assert_eq!(suggested_threshold(64_000), 35);
    assert_eq!(suggested_threshold(128_000), 25, "大窗口");
    assert_eq!(suggested_threshold(256_000), 25);
    assert_eq!(suggested_threshold(1_000_000), 15, "1M 及以上");
}

#[tokio::test]
async fn other_waterfall_points_ignored() {
    let ctx = Arc::new(Context::new());
    let cfg = config(100, CompactStrategy::Layered);
    ctx.events
        .on_waterfall(points::REQUEST, Arc::new(CtxCompact::new(ctx.clone(), cfg)));
    let payload: Value = serde_json::to_value(RequestPayload {
        messages: long_transcript(6),
        model: None,
        params: json!({}),
        tools: vec![],
        compacted: None,
        system: None,
        session_id: None,
        tree_directory: None,
    })
    .unwrap();
    let out = ctx.events.waterfall(points::PRE_STEP, payload).await;
    assert_eq!(out.decision, WaterfallDecision::Continue);
}

mod stability;
