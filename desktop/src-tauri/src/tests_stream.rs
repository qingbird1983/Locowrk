//! ⑳ 流式投影抑制：ctx 里 `LlmStreamSink` 在册 → delta 直推一份、该步
//! 整段 chunk 被抑制（正文不重复）；无 sink → 整段投影照旧（既有测试
//! `tool_turn_projects_events_in_log_order` 锁定 chunk 在场）。

use std::sync::{Arc, Mutex as StdMutex};

use kernel_agent::LlmStreamSink;
use session_store::Session;

use crate::runner::run_turn;
use crate::tests::{kernel_with_tools_streaming, Collector};

/// delta 收集 sink（模拟 StreamChunkSink 直推语义：收到即真消费）。
#[derive(Clone, Default)]
struct DeltaSink(Arc<StdMutex<Vec<String>>>);

impl LlmStreamSink for DeltaSink {
    fn on_delta(&self, _session_id: Option<&str>, delta: &str) -> bool {
        self.0.lock().unwrap().push(delta.into());
        true
    }
}

fn chunk_count(events: &[crate::events::AgentEvent]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, crate::events::AgentEvent::Chunk { .. }))
        .count()
}

#[tokio::test]
async fn streamed_step_suppresses_full_chunk() {
    let deltas = DeltaSink::default();
    let kernel = kernel_with_tools_streaming(Some(Arc::new(deltas.clone())));
    let projection = Collector::default();
    let mut session = Session::new();

    run_turn(
        &kernel,
        &mut session,
        "s1",
        "echo 你好".into(),
        None,
        &projection,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
    )
    .await
    .expect("turn ok");

    // delta 直推发生（DemoLlm 缺省流式 = 整段一次回调，两个 assistant 步）
    assert!(!deltas.0.lock().unwrap().is_empty());
    // 投影零 chunk：同一正文不出现第二遍；工具卡与收束不受影响
    assert_eq!(chunk_count(&projection.events()), 0);
    assert!(projection
        .events()
        .iter()
        .any(|e| matches!(e, crate::events::AgentEvent::TurnDone { .. })));
    // 日志正文完整（投影/落盘只认返回值）
    assert!(session.events().iter().any(|e| matches!(
        e,
        session_store::SessionEvent::AssistantMessage { content, .. } if !content.is_empty()
    )));
}

#[tokio::test]
async fn no_sink_keeps_full_chunk_projection() {
    let kernel = kernel_with_tools_streaming(None);
    let projection = Collector::default();
    let mut session = Session::new();

    run_turn(
        &kernel,
        &mut session,
        "s1",
        "echo 你好".into(),
        None,
        &projection,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
    )
    .await
    .expect("turn ok");

    assert!(
        chunk_count(&projection.events()) >= 1,
        "无 sink 路径整段投影在场"
    );
}
