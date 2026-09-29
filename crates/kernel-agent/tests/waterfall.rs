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

    assert_eq!(outcome, StepOutcome::Continue);
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
