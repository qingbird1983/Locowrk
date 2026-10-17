//! 阶段四验收：waterfall 三决策点（§7.2）——pre-step 拒绝、request 改写
//! 到达 LLM、request-error 重试与重试预算。

use std::sync::Arc;
use std::time::Duration;

use cordis_core::Context;
use kernel_agent::points;
use kernel_agent::{KernelError, LlmError, LlmService, RetryBudget, StepOutcome, ToolSource};
use plugin_api::SideEffect;
use serde_json::json;
use session_store::SessionEvent;

mod common;

use common::{
    MockSource, MockTool, ModeRewriter, PreStepRejector, RetryDecider, SeqLlm, kernel, user_session,
};

#[tokio::test]
async fn pre_step_reject_aborts_before_llm() {
    let ctx = Arc::new(Context::new());
    let llm = Arc::new(SeqLlm::new(vec![SeqLlm::ok_text("never")]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.events
        .on_waterfall(points::PRE_STEP, Arc::new(PreStepRejector));

    let mut session = user_session();
    let err = kernel(&ctx).step(&mut session).await.unwrap_err();

    assert!(
        matches!(&err, KernelError::Rejected { point, .. } if *point == points::PRE_STEP),
        "{err}"
    );
    assert_eq!(llm.requests.lock().unwrap().len(), 0, "拒绝发生在 LLM 之前");
    assert_eq!(session.len(), 1, "pre-step 拒绝不产生任何事件");
}

#[tokio::test]
async fn request_rewrite_reaches_llm() {
    let ctx = Arc::new(Context::new());
    let echo = Arc::new(MockTool::new("echo", SideEffect::Pure, json!("ECHOED")));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MockSource::new([Arc::clone(&echo)])));
    let llm = Arc::new(SeqLlm::new(vec![SeqLlm::call_tool(1, "echo")]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.events
        .on_waterfall(points::REQUEST, Arc::new(ModeRewriter));

    let mut session = user_session();
    kernel(&ctx).step(&mut session).await.unwrap();

    let requests = llm.requests.lock().unwrap();
    assert_eq!(
        requests[0].params.get("mode").and_then(|v| v.as_str()),
        Some("upper"),
        "决策环的改写必须到达 LLM 请求"
    );
    assert_eq!(echo.calls(), 1);
    // 会话以成功 ToolResult 闭合
    assert!(matches!(
        session.events().last(),
        Some(SessionEvent::ToolResult { error: None, .. })
    ));
}

#[tokio::test]
async fn request_error_retry_eventually_succeeds() {
    let ctx = Arc::new(Context::new());
    let llm = Arc::new(SeqLlm::new(vec![
        SeqLlm::ok_text("recovered"),
        Err(LlmError::Backend("boom".into())),
    ]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.events
        .on_waterfall(points::REQUEST_ERROR, Arc::new(RetryDecider));

    let mut session = user_session();
    let outcome = kernel(&ctx).step(&mut session).await.unwrap();

    assert_eq!(
        outcome,
        StepOutcome::Continue {
            streamed: false,
            reasoned: false
        }
    );
    assert_eq!(llm.requests.lock().unwrap().len(), 2, "失败一次 + 重试一次");
    assert!(matches!(
        session.events().last(),
        Some(SessionEvent::AssistantMessage { content, .. }) if content == "recovered"
    ));
}

#[tokio::test]
async fn request_error_retry_budget_exhausts() {
    let ctx = Arc::new(Context::new());
    let llm = Arc::new(SeqLlm::new(vec![]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.events
        .on_waterfall(points::REQUEST_ERROR, Arc::new(RetryDecider));

    let k = kernel(&ctx).with_retry_budget(RetryBudget {
        max_retries: 1,
        max_total_delay: Duration::from_secs(1),
    });
    let mut session = user_session();
    let err = k.step(&mut session).await.unwrap_err();

    assert!(
        matches!(err, KernelError::RetryBudgetExhausted { retries: 2, .. }),
        "初次 + 1 次重试后耗尽: {err}"
    );
    assert_eq!(llm.requests.lock().unwrap().len(), 2);
}

// ---------- 台账 T2：`agent/request` 的取还形状 ----------

/// 守卫的地基：一个字段都没碰 ⇒ 写回的字节必须与输入**逐字节相同**。
/// 这条撑着两件事——① 各模块测试里"进出 payload 相等 = 本环零参与"那种断言；
/// ② 供应商前缀缓存：若 round-trip 会重排键序或把 `None` 补成 `null`，七环连跑
/// 就等于每次请求重写一遍 system，M1 那条字节纪律当场作废。
#[test]
fn guard_writes_back_identical_bytes_when_nothing_changed() {
    let input = serde_json::to_value(kernel_agent::RequestPayload {
        messages: vec![session_store::Message::user("hi")],
        model: None,
        // 故意打乱键序并放一个 null 值：round-trip 若重排/丢字段，这里就会露出来
        params: json!({ "temperature": 0.5, "z": 1, "top_p": null }),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: Some("s1".into()),
        tree_directory: Some(String::new()),
    })
    .unwrap();
    let mut payload = input.clone();
    {
        let guard = kernel_agent::take_request(&mut payload, "test").expect("可解析");
        drop(guard);
    }
    assert_eq!(payload, input, "空改写必须逐字节原样还");
}

/// 故意**不走守卫**的环：自己把载荷 replace 走却不还。
struct Naked;

#[async_trait::async_trait]
impl cordis_core::WaterfallListener for Naked {
    async fn handle(
        &self,
        _key: &str,
        payload: &mut serde_json::Value,
    ) -> cordis_core::WaterfallDecision {
        let parsed = serde_json::from_value::<kernel_agent::RequestPayload>(std::mem::replace(
            payload,
            serde_json::Value::Null,
        ));
        assert!(parsed.is_ok(), "测试前提：进来的确实是 request 载荷");
        cordis_core::WaterfallDecision::Continue // 明知 payload 已是 Null 却放行
    }
}

/// 漏还要被下一环当场响（这条是"为什么用 RAII 守卫而不是手写 take+put"的反面证明）。
///
/// 原形状下 `Null` 会被后面每一环的 `else { Continue }` 一路放行到出环才失败；
/// 现在第一个用共用形状的环就拒，并把"是谁先发现的"报出来。
#[tokio::test]
async fn a_ring_that_forgets_to_give_back_is_caught_by_the_next_ring() {
    use kernel_agent::{BaseSystemInject, points};
    let bus = cordis_core::EventBus::new();
    bus.on_waterfall(points::REQUEST, Arc::new(Naked));
    bus.on_waterfall(points::REQUEST, Arc::new(BaseSystemInject));
    let payload = serde_json::to_value(kernel_agent::RequestPayload {
        messages: vec![session_store::Message::user("hi")],
        model: None,
        params: json!({}),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: None,
        tree_directory: None,
    })
    .unwrap();

    let out = bus.waterfall(points::REQUEST, payload).await;
    match out.decision {
        cordis_core::WaterfallDecision::Reject(r) => {
            assert_eq!(r.by, "BaseSystemInject", "该点名第一个用共用形状的环");
        }
        other => panic!("漏还该被下一环当场拒，实际 {other:?}"),
    }
}
