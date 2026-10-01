//! ⑳ 流式通道：sink 在册走 complete_streaming（delta 逐段回调、
//! StepOutcome.streamed 置位）；缺席走 complete（streamed=false）；
//! child_kernel 的 scoped 空实现压制全局 sink（子内黑盒不泄漏）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{Kernel, LlmError, LlmRequest, LlmResponse, LlmService, LlmStreamSink};
use session_store::{Session, SessionEvent};

/// 固定回复 LLM：记录 complete / complete_streaming 各被调几次。
struct CountingLlm {
    complete_calls: AtomicUsize,
    streaming_calls: AtomicUsize,
}

#[async_trait]
impl LlmService for CountingLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.complete_calls.fetch_add(1, Ordering::Relaxed);
        Ok(LlmResponse {
            content: "非流式正文".into(),
            tool_calls: vec![],
        })
    }

    async fn complete_streaming(
        &self,
        _req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.streaming_calls.fetch_add(1, Ordering::Relaxed);
        on_delta("流");
        on_delta("式");
        Ok(LlmResponse {
            content: "流式正文".into(),
            tool_calls: vec![],
        })
    }
}

/// 收集 delta 的 sink（记录 session_id 是否带出）。
struct RecordedSink(Mutex<Vec<(Option<String>, String)>>);

impl LlmStreamSink for RecordedSink {
    fn on_delta(&self, session_id: Option<&str>, delta: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .push((session_id.map(|s| s.to_string()), delta.into()));
        true
    }
}

fn user_session() -> Session {
    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "你好".into(),
    });
    s
}

#[tokio::test]
async fn sink_present_streams_and_sets_flag() {
    let ctx = Arc::new(Context::new());
    let llm = Arc::new(CountingLlm {
        complete_calls: AtomicUsize::new(0),
        streaming_calls: AtomicUsize::new(0),
    });
    ctx.provide_dyn::<dyn LlmService>(llm.clone() as Arc<dyn LlmService>);
    let sink = Arc::new(RecordedSink(Mutex::new(Vec::new())));
    ctx.provide_dyn::<dyn LlmStreamSink>(sink.clone() as Arc<dyn LlmStreamSink>);

    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);
    let mut session = user_session();
    let outcome = kernel.step_as(&mut session, "s1").await.unwrap();

    assert_eq!(
        outcome,
        kernel_agent::StepOutcome::Continue { streamed: true }
    );
    assert_eq!(llm.streaming_calls.load(Ordering::Relaxed), 1);
    assert_eq!(llm.complete_calls.load(Ordering::Relaxed), 0);
    // delta 逐段带会话身份回调
    let got = sink.0.lock().unwrap().clone();
    assert_eq!(
        got,
        vec![
            (Some("s1".into()), "流".into()),
            (Some("s1".into()), "式".into())
        ]
    );
    // 日志只认返回值：AssistantMessage 全文一份，delta 不落盘
    assert!(matches!(
        session.events().last(),
        Some(SessionEvent::AssistantMessage { content, .. }) if content == "流式正文"
    ));
}

#[tokio::test]
async fn sink_absent_falls_back_to_complete() {
    let ctx = Arc::new(Context::new());
    let llm = Arc::new(CountingLlm {
        complete_calls: AtomicUsize::new(0),
        streaming_calls: AtomicUsize::new(0),
    });
    ctx.provide_dyn::<dyn LlmService>(llm.clone() as Arc<dyn LlmService>);

    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);
    let mut session = user_session();
    let outcome = kernel.step(&mut session).await.unwrap();

    assert_eq!(
        outcome,
        kernel_agent::StepOutcome::Continue { streamed: false }
    );
    assert_eq!(llm.complete_calls.load(Ordering::Relaxed), 1);
    assert_eq!(llm.streaming_calls.load(Ordering::Relaxed), 0);
}
