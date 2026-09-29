//! 阶段四验收：审批门（纪律 9 / §7.2）——Irreversible 工具调用前必须审批，
//! 决策作为 ApprovalDecided 事件进日志；被拒调用当场闭合、不留悬空。

use std::sync::Arc;

use cordis_core::Context;
use kernel_agent::{LlmService, ToolSource};
use plugin_api::SideEffect;
use serde_json::json;
use session_store::SessionEvent;

mod common;

use common::{AllowAll, DenyAll, MockSource, MockTool, SeqLlm, kernel, user_session};

#[tokio::test]
async fn irreversible_denied_without_resolver() {
    let ctx = Arc::new(Context::new());
    let bomb = Arc::new(MockTool::new(
        "danger",
        SideEffect::Irreversible,
        json!("destroyed"),
    ));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MockSource::new([Arc::clone(&bomb)])));
    let llm = Arc::new(SeqLlm::new(vec![SeqLlm::call_tool(1, "danger")]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);

    let mut session = user_session();
    kernel(&ctx).step(&mut session).await.unwrap();

    // fail-safe：无决议者 = 拒绝；工具从未执行，调用以 denied ToolResult 闭合
    assert_eq!(bomb.calls(), 0);
    let events = session.events();
    assert!(matches!(
        events.get(events.len() - 2),
        Some(SessionEvent::ApprovalDecided { allowed: false, .. })
    ));
    assert!(matches!(
        events.last(),
        Some(SessionEvent::ToolResult { error: Some(e), .. }) if e.contains("denied")
    ));
}

#[tokio::test]
async fn irreversible_allowed_via_resolver_executes() {
    let ctx = Arc::new(Context::new());
    let bomb = Arc::new(MockTool::new(
        "danger",
        SideEffect::Irreversible,
        json!("destroyed"),
    ));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MockSource::new([Arc::clone(&bomb)])));
    let llm = Arc::new(SeqLlm::new(vec![SeqLlm::call_tool(1, "danger")]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);

    let mut session = user_session();
    kernel(&ctx)
        .with_approval(Arc::new(AllowAll))
        .step(&mut session)
        .await
        .unwrap();

    assert_eq!(bomb.calls(), 1, "批准后工具执行");
    assert!(matches!(
        session
            .events()
            .iter()
            .rev()
            .find(|e| matches!(e, SessionEvent::ApprovalDecided { .. })),
        Some(SessionEvent::ApprovalDecided { allowed: true, .. })
    ));
}

#[tokio::test]
async fn denied_reason_lands_in_approval_event() {
    let ctx = Arc::new(Context::new());
    let bomb = Arc::new(MockTool::new(
        "danger",
        SideEffect::Irreversible,
        json!("destroyed"),
    ));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MockSource::new([Arc::clone(&bomb)])));
    let llm = Arc::new(SeqLlm::new(vec![SeqLlm::call_tool(1, "danger")]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);

    let mut session = user_session();
    kernel(&ctx)
        .with_approval(Arc::new(DenyAll("needs human review".into())))
        .step(&mut session)
        .await
        .unwrap();

    assert_eq!(bomb.calls(), 0);
    assert!(matches!(
        session.events().iter().rev().find(|e| matches!(
            e,
            SessionEvent::ApprovalDecided { .. }
        )),
        Some(SessionEvent::ApprovalDecided {
            allowed: false,
            reason: Some(reason),
            ..
        }) if reason == "needs human review"
    ));
}
