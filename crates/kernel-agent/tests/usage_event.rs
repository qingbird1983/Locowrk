//! v2.58 §4.1 用量事件回归：`SessionEvent::LlmUsage` 紧贴 AssistantMessage
//! 之后落日志、derive_messages 跳过、`LlmRequest.session_id` 投影（自
//! react_loop.rs 按单文件上限拆出——拆分是独立任务、语义不变）。

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{Kernel, LlmError, LlmRequest, LlmResponse, LlmService};
use session_store::{Session, SessionEvent};

/// v2.58 §4.1：供应商报告用量 → `SessionEvent::LlmUsage` 紧贴
/// AssistantMessage 之后落日志；derive_messages 跳过（过程计量不是
/// 上下文）；不报告 = 不落事件。顺带锁 `LlmRequest.session_id` 投影
/// （`RequestPayload` 已有，此前丢失——桌面端会话级模型覆盖靠它）。
#[tokio::test]
async fn usage_lands_as_event_and_session_id_reaches_request() {
    struct UsageLlm;
    #[async_trait]
    impl LlmService for UsageLlm {
        async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
            assert_eq!(req.session_id.as_deref(), Some("desktop-7"));
            Ok(LlmResponse {
                content: "done".into(),
                tool_calls: vec![],
                usage: Some(kernel_agent::LlmUsage {
                    input: 100,
                    output: 20,
                    cache_read: 80,
                    cache_write: 0,
                    reasoning: 5,
                    total: 125,
                }),
            })
        }
    }

    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn LlmService>(Arc::new(UsageLlm));
    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);

    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "hi".into(),
    });
    kernel.step_as(&mut session, "desktop-7").await.unwrap();

    // 紧贴 AssistantMessage 之后，全字段如实
    let events = session.events();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[1], SessionEvent::AssistantMessage { .. }));
    let SessionEvent::LlmUsage {
        input,
        output,
        cache_read,
        reasoning,
        total,
        duration_ms,
        ..
    } = &events[2]
    else {
        panic!("expected LlmUsage, got {:?}", events[2]);
    };
    assert_eq!(
        (input, output, cache_read, reasoning, total),
        (&100, &20, &80, &5, &125)
    );
    assert!(*duration_ms < 5_000, "duration should be request wall time");

    // derive_messages 跳过：上下文仍然只有 user + assistant
    assert_eq!(session.derive_messages().len(), 2);
}
