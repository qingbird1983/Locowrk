//! ⑳ 流式投影抑制：ctx 里 `LlmStreamSink` 在册 → delta 直推一份、该步
//! 整段 chunk 被抑制（正文不重复）；无 sink → 整段投影照旧（既有测试
//! `tool_turn_projects_events_in_log_order` 锁定 chunk 在场）。

use std::sync::{Arc, Mutex as StdMutex};

use kernel_agent::LlmStreamSink;
use session_store::Session;

use crate::runner::run_turn;
use crate::tests::{kernel_with_tools, kernel_with_tools_streaming, Collector};

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
        Vec::new(),
        None,
        &projection,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
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
        Vec::new(),
        None,
        &projection,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    assert!(
        chunk_count(&projection.events()) >= 1,
        "无 sink 路径整段投影在场"
    );
}

/// v2.57 §3.9 推理投影：Thinking 事件进全量投影（重放行）；实时路径
/// `reasoned` 置位时整段被抑制（delta 已直推，不重复）；UserMessage 投影
/// 带事件位 pos（§3.34 编辑重发的定位键）。
#[test]
fn thinking_projects_on_replay_and_suppresses_when_streamed() {
    use session_store::SessionEvent;

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "问".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::Thinking {
        text: "想一步".into(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "答".into(),
        tool_calls: vec![],
    });

    // 重放（session_history 全量）：Thinking 行在场；pos = 源事件位
    let replay = crate::runner::project_events(s.events(), "s1");
    assert!(replay.iter().any(
        |e| matches!(e, crate::events::AgentEvent::Thinking { text, .. } if text == "想一步")
    ));
    assert!(replay.iter().any(|e| matches!(
        e,
        crate::events::AgentEvent::UserMessage { content, pos, .. } if content == "问" && *pos == 0
    )));

    // 实时（reasoned = true）：整段 Thinking 抑制，其余照旧
    let sink = Collector::default();
    crate::runner::project_new_events(&s, 0, "s1", &sink, false, true);
    assert!(!sink
        .events()
        .iter()
        .any(|e| matches!(e, crate::events::AgentEvent::Thinking { .. })));
    let sink2 = Collector::default();
    crate::runner::project_new_events(&s, 0, "s1", &sink2, false, false);
    assert!(sink2
        .events()
        .iter()
        .any(|e| matches!(e, crate::events::AgentEvent::Thinking { .. })));
}

/// v2.58 §3.12 用量投影：`SessionEvent::LlmUsage` 进全量投影（实时与
/// 重放同形），全字段如实透传；ContextUsage 载荷带目录估算。
#[test]
fn llm_usage_projects_with_all_fields() {
    use session_store::SessionEvent;

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "问".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "答".into(),
        tool_calls: vec![],
    });
    s.append(SessionEvent::LlmUsage {
        input: 100,
        output: 20,
        cache_read: 80,
        cache_write: 4,
        reasoning: 5,
        total: 125,
        duration_ms: 1234,
    });

    let replay = crate::runner::project_events(s.events(), "s1");
    let hit = replay.iter().find_map(|e| match e {
        crate::events::AgentEvent::LlmUsage {
            input,
            output,
            cache_read,
            cache_write,
            reasoning,
            total,
            duration_ms,
            ..
        } => Some((
            *input,
            *output,
            *cache_read,
            *cache_write,
            *reasoning,
            *total,
            *duration_ms,
        )),
        _ => None,
    });
    assert_eq!(hit, Some((100, 20, 80, 4, 5, 125, 1234)));
}

/// v2.101 轮界事件投影：TurnStart.ts 配给紧随的 UserMessage（重放计时
/// 起点），TurnEnd 映成带时刻的 TurnDone（steps 日志未记录 = 不带）；
/// 旧格式日志（无轮界事件）投影 ts 缺省——前端诚实省略读数，不虚构。
#[test]
fn turn_boundaries_project_timestamps_for_replay() {
    use session_store::{SessionEvent, TurnId, TurnOutcome};

    let mut s = Session::new();
    s.append(SessionEvent::TurnStart {
        turn_id: TurnId(1),
        ts: 1_000,
    });
    s.append(SessionEvent::UserMessage {
        content: "问".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "答".into(),
        tool_calls: vec![],
    });
    s.append(SessionEvent::TurnEnd {
        turn_id: TurnId(1),
        outcome: TurnOutcome::Completed,
        ts: 61_000,
    });

    let replay = crate::runner::project_events(s.events(), "s1");
    assert!(
        replay.iter().any(|e| matches!(
            e,
            crate::events::AgentEvent::UserMessage {
                ts: Some(1_000),
                ..
            }
        )),
        "UserMessage 应带上 TurnStart 的轮起点：{replay:?}"
    );
    assert!(
        replay.iter().any(|e| matches!(
            e,
            crate::events::AgentEvent::TurnDone {
                steps: None,
                ts: Some(61_000),
                ..
            }
        )),
        "TurnEnd 应映成带时刻、不带 steps 的 TurnDone：{replay:?}"
    );

    // 旧格式（无轮界事件）：ts 缺省
    let mut old = Session::new();
    old.append(SessionEvent::UserMessage {
        content: "旧".into(),
        parts: Vec::new(),
    });
    let replay_old = crate::runner::project_events(old.events(), "s1");
    assert!(replay_old
        .iter()
        .any(|e| matches!(e, crate::events::AgentEvent::UserMessage { ts: None, .. })));
}

/// v2.101：run_turn 把轮界事件写进日志（此前 §4.1 已定义但从未写入，
/// 重放无从复原计时读数），实时 TurnDone 与 TurnEnd 时刻同源。
#[tokio::test]
async fn run_turn_writes_turn_boundaries_to_log() {
    use session_store::{SessionEvent, TurnOutcome};

    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s1",
        "echo 你好".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    assert!(
        matches!(
            session.events().first(),
            Some(SessionEvent::TurnStart { ts, .. }) if *ts > 0
        ),
        "轮首应落 TurnStart：{:?}",
        session.events().first()
    );
    assert!(
        matches!(
            session.events().last(),
            Some(SessionEvent::TurnEnd {
                outcome: TurnOutcome::Completed,
                ts,
                ..
            }) if *ts > 0
        ),
        "轮尾应落 TurnEnd(Completed)：{:?}",
        session.events().last()
    );
    assert!(
        sink.events().iter().any(|e| matches!(
            e,
            crate::events::AgentEvent::TurnDone {
                steps: Some(_),
                ts: Some(_),
                ..
            }
        )),
        "实时 TurnDone 带步数与收束时刻"
    );
}
