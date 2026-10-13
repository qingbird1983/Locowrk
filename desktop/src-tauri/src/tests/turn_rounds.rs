//! 轮次投影与收束（自 `tests.rs` 批 F-17 拆出，用例本体逐字未改）：工具轮的日志事件
//! 次序（tool_call → tool_result → chunk → turn_done）/ 纯文本轮单步收束 / 步数上限
//! 到顶照常 TurnDone 并如实发 TurnCapped。

use super::*;

use crate::events::AgentEvent;
use crate::runner::run_turn;
use session_store::Session;

#[tokio::test]
async fn tool_turn_projects_events_in_log_order() {
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
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
    assert_eq!(steps, 2);

    let events = sink.events();
    // 步1：AssistantMessage 正文为空（无 chunk）→ ToolCall → ToolResult；
    // 步2：汇总正文（chunk）→ TurnDone
    let kinds: Vec<&str> = events.iter().map(kind).collect();
    assert_eq!(
        kinds,
        vec!["tool_call", "tool_result", "chunk", "turn_done"]
    );
    let AgentEvent::ToolResult { ok, error, .. } = &events[1] else {
        panic!("expected tool_result")
    };
    assert!(*ok && error.is_none());
    let AgentEvent::Chunk { delta, .. } = &events[2] else {
        panic!("expected chunk")
    };
    assert!(delta.contains("工具返回"));
}

/// 单轮步数上限按调用方配置收束（settings tools.maxTurnSteps 的消费端
/// 契约；LLM.md v3.0 同批改约）——第 1 步未收敛即到顶：不再 Err 硬掐，
/// TurnDone 正常收束 + TurnCapped 如实告知，已完成的工具成果保留。
#[tokio::test]
async fn step_cap_force_closes_with_configured_limit() {
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
        "echo 你好".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        1,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("到顶按正常收束返回");
    assert_eq!(steps, 1);
    let kinds: Vec<&str> = sink.events().iter().map(|e| kind(e)).collect();
    assert!(
        kinds.contains(&"turn_done"),
        "正常 TurnDone 收束：{kinds:?}"
    );
    assert!(
        kinds.contains(&"turn_capped"),
        "TurnCapped 如实告知：{kinds:?}"
    );
}

#[tokio::test]
async fn plain_reply_turn_is_single_step() {
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
        "你好".into(),
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
    assert_eq!(steps, 1);

    let kinds: Vec<&str> = sink.events().iter().map(kind).collect();
    assert_eq!(kinds, vec!["chunk", "turn_done"]);
}
