//! `ToolStatus`（后审计队列 Q12·C1）的边界用例：**它只属于实时**。
//!
//! 这条用例钉的是取证批 G4 那一处「`pending_approval` 重放不可复原」的处置：
//! 等待态的真源是「卡片正挂着」这件事，它不落会话日志——重放时那次决策早已
//! 落成 `ApprovalDecided`，行按"经审批放行/拒绝"的事实闭合。把等待态写进日志
//! 反而要处理「请求了却没回执」的悬空形状（崩溃重放时那条怎么闭合），而它
//! 对用户没有任何新增信息。所以这里同时断言两面：实时推得出、日志里没有。

use super::*;

use crate::approval::ApprovalHub;
use crate::events::AgentEvent;
use crate::runner::projection::project_events;
use crate::runner::run_turn;
use crate::tests::{kind, Collector};
use session_store::{Session, SessionEvent};
use std::sync::Arc;

#[tokio::test]
async fn pending_approval_is_live_only_and_never_enters_the_log() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::new(Arc::new(sink.clone())));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    let decider = Arc::clone(&hub);
    Decider::new(decider).spawn(|_| {});

    bounded(
        "run_turn（活态投影片要点卡）",
        DECIDER_BUDGET,
        run_turn(
            &kernel,
            &mut session,
            "s-live",
            "发邮件".into(),
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
        ),
    )
    .await
    .expect("turn ok");

    // ① 实时：请求一发出就补投等待态，call_id 与审批载荷同串（一份标识两个用途）
    let request_id = sink
        .events()
        .iter()
        .find_map(|e| match e {
            AgentEvent::ApprovalRequest(p) => Some(p.request_id.clone()),
            _ => None,
        })
        .expect("approval_request 在场");
    let statuses: Vec<(String, String)> = sink
        .events()
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolStatus {
                call_id, status, ..
            } => Some((call_id.clone(), status.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        statuses.as_slice(),
        &[(request_id.clone(), "pending_approval".to_string())],
        "每次审批请求补投恰好一条等待态（多投 = 同一事实两个出处）"
    );

    // ② 顺序：等待态在审批请求之后、结果之前（前端据此把行点亮再收起）
    let order: Vec<&str> = sink.events().iter().map(kind).collect();
    let req_at = order.iter().position(|k| *k == "approval_request").unwrap();
    let st_at = order.iter().position(|k| *k == "tool_status").unwrap();
    let res_at = order.iter().position(|k| *k == "tool_result").unwrap();
    assert!(
        req_at < st_at && st_at < res_at,
        "顺序应 request → status → result，实得 {order:?}"
    );

    // ③ 日志面：等待态不落盘——日志里只有"决策"这一条事实
    let decided = session
        .events()
        .iter()
        .filter(|e| matches!(e, SessionEvent::ApprovalDecided { .. }))
        .count();
    assert_eq!(decided, 1, "日志里只有决策那一条（等待态不落盘）");

    // ④ 重放面：同一段日志投影出来的事件序列里 tool_status 出现 0 次
    let replay: Vec<String> = project_events(session.events(), "s-live")
        .iter()
        .map(|e| kind(e).to_string())
        .collect();
    assert_eq!(
        replay.iter().filter(|k| **k == "tool_status").count(),
        0,
        "重放不得虚构等待态，实得 {replay:?}"
    );
    assert!(
        replay.contains(&"approval_resolved".to_string()),
        "重放照常给出决策记录条，实得 {replay:?}"
    );
}
