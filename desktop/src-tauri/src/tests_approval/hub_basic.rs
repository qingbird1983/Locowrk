//! 审批门基本回路（自 `tests_approval.rs` 批 F-14 拆出，用例本体逐字未改）：拒绝即闭合不执行 /
//! 放行执行并落决议事件 / ApprovalHub 经 pending map 端到端 / 未知 request 报错。

use super::*;

use crate::approval::ApprovalHub;
use crate::events::AgentEvent;
use crate::runner::run_turn;
use crate::tests::{kind, Collector};
use async_trait::async_trait;
use kernel_agent::{ApprovalDecision, ApprovalResolver};
use plugin_api::ToolSpec;
use serde_json::json;
use session_store::{Session, SessionEvent};
use std::sync::Arc;

/// 固定决议的审批替身：不推事件、不等待，直接返回预设决策。
struct FixedResolver(ApprovalDecision);

#[async_trait]
impl ApprovalResolver for FixedResolver {
    async fn resolve(
        &self,
        _req: &kernel_agent::ToolCallReq,
        _spec: &ToolSpec,
    ) -> ApprovalDecision {
        self.0.clone()
    }
}

#[tokio::test]
async fn denied_email_closes_result_without_executing() {
    let kernel = kernel_with_email(Arc::new(FixedResolver(ApprovalDecision::deny("测试拒绝"))));
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
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
    )
    .await
    .expect("turn ok");
    assert_eq!(steps, 2);

    // 决策进日志（纪律 9），被拒调用以失败 ToolResult 闭合、不留悬空
    let decided = session.events().iter().any(|e| {
        matches!(
            e,
            SessionEvent::ApprovalDecided { allowed: false, reason: Some(r), .. } if r == "测试拒绝"
        )
    });
    assert!(decided, "ApprovalDecided 必须写入会话日志");
    let closed = session.events().iter().any(|e| {
        matches!(
            e,
            SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")
        )
    });
    assert!(closed, "被拒调用必须以 denied 的 ToolResult 闭合");

    // UI 投影：卡片先出现（tool_call），随后决策记录条 + 失败（tool_result）
    // v2.205：轮末多出 `turn_header`（本批新增事件的正常结果，非行为回归）。
    let kinds: Vec<&str> = sink.events().iter().map(kind).collect();
    assert_eq!(
        kinds,
        vec![
            "tool_call",
            "approval_resolved",
            "tool_result",
            "chunk",
            "turn_header",
            "turn_done"
        ]
    );
}

#[tokio::test]
async fn allowed_email_executes_and_logs_decision() {
    let kernel = kernel_with_email(Arc::new(FixedResolver(ApprovalDecision::allow())));
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s1",
        "email 给我".into(),
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

    assert!(session
        .events()
        .iter()
        .any(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: true, .. })));
    let events = sink.events();
    let AgentEvent::ToolResult { ok, output, .. } = events
        .iter()
        .find(|e| kind(e) == "tool_result")
        .expect("expected tool_result")
    else {
        panic!("expected tool_result")
    };
    assert!(*ok);
    assert_eq!(output["sent"], json!(true));
}

/// ApprovalHub 端到端（不含 Tauri）：请求挂起 → 后台决策 → step 醒来闭合。
#[tokio::test]
async fn approval_hub_roundtrip_through_pending_map() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::new(Arc::new(sink.clone())));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    // 后台决策者：轮询 pending 表，看到请求后拒绝（模拟用户点"拒绝"）
    let decider = Arc::clone(&hub);
    Decider::new(decider)
        .deny()
        .reason("用户拒绝")
        .spawn(|_| {});

    bounded(
        "run_turn（端到端要点卡）",
        DECIDER_BUDGET,
        run_turn(
            &kernel,
            &mut session,
            "s1",
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

    // 前端收到审批请求事件；轮次以失败 ToolResult 收束而非卡死
    assert!(
        sink.events().iter().any(|e| kind(e) == "approval_request"),
        "approval_request 事件必须推给前端"
    );
    // ⑮：载荷携带会话身份（子 Agent 派生的调用据此打标识徽章）
    let payload = sink.events().iter().find_map(|e| match e {
        AgentEvent::ApprovalRequest(p) => Some(p.clone()),
        _ => None,
    });
    assert_eq!(
        payload.expect("审批载荷").session_id.as_deref(),
        Some("s1"),
        "⑮ 审批载荷带会话身份"
    );
    assert!(session.events().iter().any(|e| matches!(
        e,
        SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")
    )));
    // 决策后 pending 表必须清空（list_pending_approvals 不残留）
    assert!(hub.pending().is_empty());
}

#[tokio::test]
async fn deciding_unknown_request_is_an_error() {
    let hub = ApprovalHub::new(Arc::new(Collector::default()));
    assert!(hub.decide("call-404", true, None, None).is_err());
}
