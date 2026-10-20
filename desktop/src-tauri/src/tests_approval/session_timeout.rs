//! 会话豁免与超时守夜（自 `tests_approval.rs` 批 F-14 拆出，用例本体逐字未改）：
//! scope=session 放行后同会话同工具豁免 / 短超时到期 fail-closed 拒绝 / 无限超时继续等待。

use super::*;

use crate::approval::ApprovalHub;
use crate::events::AgentEvent;
use crate::runner::run_turn;
use crate::tests::{kind, Collector};
use kernel_agent::ApprovalResolver;
use session_store::{Session, SessionEvent};
use std::sync::Arc;

/// ㉕·对话区① §3.17「允许本次对话」：scope=session 放行后，同会话同工具
/// 的下一次调用豁免命中——不再推 approval-request；别的会话照问。
#[tokio::test]
async fn session_scope_exempts_same_tool_next_call() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::new(Arc::new(sink.clone())));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    // 第一轮：看到请求后按「允许本次对话」放行
    let decider = Arc::clone(&hub);
    Decider::new(decider).scope("session").spawn(|_| {});
    // 这一步要靠决断者回填才收束（本 hub = 测试缺省 100s 守夜）⇒ 包上界，
    // 卡片没出现时 10 秒内报"挂起"，而不是等满 100 秒再由守夜伪装成超时拒绝。
    bounded(
        "run_turn（第一轮要点卡）",
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
    .expect("turn 1 ok");
    assert_eq!(
        sink.events()
            .iter()
            .filter(|e| kind(e) == "approval_request")
            .count(),
        1,
        "第一轮必须推过一次请求"
    );

    // 第二轮：豁免命中，不再问人（不新增 approval_request）
    let asked_before = sink
        .events()
        .iter()
        .filter(|e| kind(e) == "approval_request")
        .count();
    run_turn(
        &kernel,
        &mut session,
        "s1",
        "再发一封邮件".into(),
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
    .expect("turn 2 ok");
    let asked_after = sink
        .events()
        .iter()
        .filter(|e| kind(e) == "approval_request")
        .count();
    assert_eq!(asked_after, asked_before, "豁免命中不得再推请求");
    // 放行照写审计：两条 ApprovalDecided（每轮一条）
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: true, .. }))
            .count(),
        2,
        "豁免放行也必须逐条落 ApprovalDecided"
    );
}

/// ㉕·对话区① §3.17 超时自动拒绝：短超时守夜到期 → fail-closed 拒绝收束，
/// ApprovalDecided 带超时理由落日志。
#[tokio::test]
async fn timeout_guard_denies_when_unanswered() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::with_timeout(
        Arc::new(sink.clone()),
        std::time::Duration::from_millis(50),
    ));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

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
    )
    .await
    .expect("turn ok（超时按拒绝收束，不卡死）");

    assert!(hub.pending().is_empty(), "守夜必须清掉 pending 表");
    let timed_out = session.events().iter().any(|e| {
        matches!(
            e,
            SessionEvent::ApprovalDecided { allowed: false, reason: Some(r), .. }
                if r.contains("超时自动拒绝")
        )
    });
    assert!(timed_out, "超时拒绝必须带 fail-closed 理由落日志");
    assert!(
        session.events().iter().any(|e| matches!(
            e,
            SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")
        )),
        "超时拒绝的调用以失败 ToolResult 闭合"
    );
}

/// v2.49 可配置原则：`tools.approvalTimeoutSecs = 0` = 不限——不起守夜
/// 任务，审批点保持等待直到用户决策（decide 手动收束）；载荷带
/// `timeout_secs` 给决策卡倒计时。守夜机制本体由
/// `timeout_guard_denies_when_unanswered` 锁定，这里锁"不限档不超时"。
#[tokio::test]
async fn unlimited_timeout_keeps_waiting_for_decision() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::with_timeout(
        Arc::new(sink.clone()),
        std::time::Duration::from_secs(0), // 0 = 不限
    ));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    // 决策者：300ms 后才放行——若守夜存在早已抢先拒绝
    let decider = Arc::clone(&hub);
    Decider::new(decider)
        .after(std::time::Duration::from_millis(300))
        .spawn(|_| {});
    // 本用例的"超时 = 0（不起守夜）"正是要锁的语义，不能为了躲挂起改短它
    // （批 F-24a 在 `tests_permission.rs` 里那种做法就是这条台账要收的账）。
    // 挂起保护放在等待这一侧：决断者有界 + 这一步有界，两边都在预算内失败。
    bounded(
        "run_turn（不限档等手动决策）",
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
    .expect("turn ok（不限档等待手动决策后收束）");

    // 载荷带 timeout_secs = 0（决策卡不渲染倒计时的依据）
    let events = sink.events();
    let req_ev = events
        .iter()
        .find(|e| kind(e) == "approval_request")
        .expect("请求已推");
    assert!(matches!(req_ev, AgentEvent::ApprovalRequest(p) if p.timeout_secs == 0));
    // 决策来自用户放行而非超时拒绝
    assert!(session
        .events()
        .iter()
        .any(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: true, .. })));
    assert!(!session.events().iter().any(|e| matches!(
        e,
        SessionEvent::ApprovalDecided { reason: Some(r), .. } if r.contains("超时自动拒绝")
    )));
}
