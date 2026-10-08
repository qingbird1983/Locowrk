//! 阶段六审批门测试（§13.2 / 纪律 9）：固定决议替身 / ApprovalHub 端到端。
//! （原在 tests.rs，400 行纪律拆出；共用收集器与 kind 从 tests 引入。）

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{ApprovalDecision, ApprovalResolver, Kernel, LlmService, ToolSource};
use plugin_api::ToolSpec;
use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::json;
use session_store::{Session, SessionEvent};

use crate::approval::ApprovalHub;
use crate::demo_llm::DemoLlm;
use crate::demo_tools::SendEmailTool;
use crate::events::AgentEvent;
use crate::runner::run_turn;
use crate::tests::{kind, Collector};
use crate::tool_source::KernelToolSource;

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

/// 带 send_email（Irreversible）+ 指定审批决议者的内核。
fn kernel_with_email(approval: Arc<dyn ApprovalResolver>) -> Kernel {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(
        KernelToolSource::new(RegistryToolSource::new(Arc::new(PluginRegistry::new())))
            .with_builtin(Arc::new(SendEmailTool)),
    ));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(DemoLlm::new()));
    Kernel::new(ctx, vec![]).with_approval(approval)
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
    let kinds: Vec<&str> = sink.events().iter().map(kind).collect();
    assert_eq!(
        kinds,
        vec![
            "tool_call",
            "approval_resolved",
            "tool_result",
            "chunk",
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
    tokio::spawn(async move {
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, false, Some("用户拒绝".into()), None)
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });

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
    tokio::spawn(async move {
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, true, None, Some("session"))
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
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
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, true, None, None)
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
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
