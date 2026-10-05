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
            usage: None,
        })
    }

    async fn complete_streaming(
        &self,
        _req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.streaming_calls.fetch_add(1, Ordering::Relaxed);
        let _ = on_delta("流");
        let _ = on_delta("式");
        Ok(LlmResponse {
            content: "流式正文".into(),
            tool_calls: vec![],
            usage: None,
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
        kernel_agent::StepOutcome::Continue {
            streamed: true,
            reasoned: false
        }
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
        kernel_agent::StepOutcome::Continue {
            streamed: false,
            reasoned: false
        }
    );
    assert_eq!(llm.complete_calls.load(Ordering::Relaxed), 1);
    assert_eq!(llm.streaming_calls.load(Ordering::Relaxed), 0);
}

/// v2.57 推理通道：带 reasoning 的适配器走 complete_streaming_reasoning，
/// sink 消费推理 delta → reasoned 置位；全文以 Thinking 事件紧贴
/// AssistantMessage 之前落日志；derive_messages 跳过（不回传供应商）。
struct ReasoningLlm;

#[async_trait]
impl LlmService for ReasoningLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: "结论".into(),
            tool_calls: vec![],
            usage: None,
        })
    }

    async fn complete_streaming_reasoning(
        &self,
        _req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let _ = on_reasoning("想");
        let _ = on_reasoning("一步");
        let _ = on_delta("结");
        Ok(LlmResponse {
            content: "结论".into(),
            tool_calls: vec![],
            usage: None,
        })
    }
}

/// 只消费推理的 sink（on_delta 缺省 false = 文本走整段投影）。
struct ReasoningSink(Mutex<Vec<String>>);

impl LlmStreamSink for ReasoningSink {
    fn on_delta(&self, _session_id: Option<&str>, _delta: &str) -> bool {
        false
    }

    fn on_reasoning(&self, _session_id: Option<&str>, delta: &str) -> bool {
        self.0.lock().unwrap().push(delta.into());
        true
    }
}

#[tokio::test]
async fn reasoning_streams_consumed_and_lands_as_thinking_event() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ReasoningLlm) as Arc<dyn LlmService>);
    let sink = Arc::new(ReasoningSink(Mutex::new(Vec::new())));
    ctx.provide_dyn::<dyn LlmStreamSink>(sink.clone() as Arc<dyn LlmStreamSink>);

    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);
    let mut session = user_session();
    let outcome = kernel.step_as(&mut session, "s1").await.unwrap();

    assert_eq!(
        outcome,
        kernel_agent::StepOutcome::Continue {
            streamed: false,
            reasoned: true
        }
    );
    assert_eq!(
        *sink.0.lock().unwrap(),
        vec!["想".to_string(), "一步".to_string()]
    );

    // 落盘时序：Thinking 紧贴 AssistantMessage 之前，全文一份
    let events = session.events();
    assert!(matches!(
        events.last(),
        Some(SessionEvent::AssistantMessage { content, .. }) if content == "结论"
    ));
    assert!(matches!(
        events.get(events.len() - 2),
        Some(SessionEvent::Thinking { text }) if text == "想一步"
    ));

    // 推理是过程不是上下文：derive_messages 不含 Thinking
    let msgs = session.derive_messages();
    assert_eq!(msgs.len(), 2, "用户消息 + 助手正文，推理不入列");
}
