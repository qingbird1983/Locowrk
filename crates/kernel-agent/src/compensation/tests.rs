//! 补偿执行器单测：假工具 + 假会话，覆盖 §6.3 矩阵——undo 记日志、账本
//! 幂等、Irreversible 原调用 → ManualIntervention、失败调用不补偿、
//! Irreversible undo 工具过审批门。

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::Context;
use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{Value, json};
use session_store::CompensatedLedger;
use session_store::{Session, SessionEvent};

use crate::approval::{ApprovalDecision, ApprovalResolver};
use crate::compensation::CompensationReport;
use crate::kernel::{Kernel, ToolCallReq, ToolSource};

/// 记录收到的 args、返回固定结果的假 undo 工具（fail=true 时执行失败）。
struct UndoTool {
    spec: ToolSpec,
    calls: Arc<Mutex<Vec<Value>>>,
    fail: bool,
}

impl UndoTool {
    fn new(name: &str, side_effect: SideEffect, calls: Arc<Mutex<Vec<Value>>>) -> Self {
        Self::with_fail(name, side_effect, calls, false)
    }

    fn with_fail(
        name: &str,
        side_effect: SideEffect,
        calls: Arc<Mutex<Vec<Value>>>,
        fail: bool,
    ) -> Self {
        Self {
            spec: ToolSpec {
                name: name.into(),
                description: "test undo".into(),
                schema: json!({ "type": "object" }),
                side_effect,
            },
            calls,
            fail,
        }
    }
}

#[async_trait]
impl Tool for UndoTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(1, 1, 0);
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        self.calls.lock().unwrap().push(args);
        if self.fail {
            return Err(ToolError::Failed {
                tool: self.spec.name.clone(),
                message: "undo blew up".into(),
            });
        }
        Ok(json!({ "restored": true }))
    }
}

struct UndoSource(Vec<Arc<UndoTool>>);

#[async_trait]
impl ToolSource for UndoSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        self.0
            .iter()
            .find(|t| t.spec.name == name)
            .map(|t| Arc::clone(t) as Arc<dyn Tool>)
            .ok_or_else(|| ToolError::NotAvailable(name.to_string()))
    }

    async fn list(&self) -> Vec<ToolSpec> {
        self.0.iter().map(|t| t.spec.clone()).collect()
    }
}

struct FixedResolver(ApprovalDecision);

#[async_trait]
impl ApprovalResolver for FixedResolver {
    async fn resolve(&self, _req: &ToolCallReq, _spec: &ToolSpec) -> ApprovalDecision {
        self.0.clone()
    }
}

fn recorder() -> Arc<Mutex<Vec<Value>>> {
    Arc::new(Mutex::new(Vec::new()))
}

fn kernel_with(tools: Vec<Arc<UndoTool>>, approval: Option<FixedResolver>) -> Kernel {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(UndoSource(tools)));
    let mut k = Kernel::new(ctx, vec![]);
    if let Some(resolver) = approval {
        k = k.with_approval(Arc::new(resolver));
    }
    k
}

/// 一条 fs.write 调用事件对；side_effect 由用例指定（plan 按事件声明判级）。
fn push_write(session: &mut Session, call_id: &str, undo_tool: &str, ok: bool) {
    push_call(
        session,
        call_id,
        SideEffect::Compensable {
            undo_tool: undo_tool.into(),
        },
        ok,
    );
}

fn push_call(session: &mut Session, call_id: &str, side_effect: SideEffect, ok: bool) {
    session.append(SessionEvent::ToolCall {
        call_id: CallId(call_id.into()),
        tool: "fs.write".into(),
        args: json!({ "path": "/tmp/note.txt", "content": "v2" }),
        plugin_version: Version::new(1, 1, 0),
        side_effect,
    });
    session.append(SessionEvent::ToolResult {
        call_id: CallId(call_id.into()),
        output: json!({ "path": "/tmp/note.txt", "existed": true, "backup": "/b/1" }),
        error: if ok { None } else { Some("boom".into()) },
    });
}

#[tokio::test]
async fn undo_runs_through_chain_and_logs() {
    let calls = recorder();
    let kernel = kernel_with(
        vec![Arc::new(UndoTool::new(
            "fs.restore",
            SideEffect::Pure,
            Arc::clone(&calls),
        ))],
        None,
    );
    let mut session = Session::new();
    push_write(&mut session, "call-1", "fs.restore", true);

    let mut ledger = CompensatedLedger::default();
    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("compensate ok");

    assert_eq!(report.undone.len(), 1);
    assert_eq!(report.undone[0].0, "call-1");
    assert!(ledger.contains(&CallId("call-1".into())));

    // undo 调用按纪律 5 记日志：新 call_id（原 id-undo）、契约三件套齐全
    let events = session.events();
    let undo_call = events.iter().any(|e| {
        matches!(e, SessionEvent::ToolCall { call_id, tool, args, .. }
            if call_id.0 == "call-1-undo" && tool == "fs.restore"
                && args["call_id"] == json!("call-1")
                && args["output"]["backup"] == json!("/b/1"))
    });
    assert!(
        undo_call,
        "undo ToolCall must carry call_id + args + output"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, SessionEvent::ToolResult { call_id, error: None, .. }
            if call_id.0 == "call-1-undo")
        ),
        "successful undo closes its ToolResult"
    );
    assert_eq!(calls.lock().unwrap().len(), 1, "undo tool executed once");
}

#[tokio::test]
async fn ledger_blocks_second_compensation() {
    let calls = recorder();
    let kernel = kernel_with(
        vec![Arc::new(UndoTool::new(
            "fs.restore",
            SideEffect::Pure,
            Arc::clone(&calls),
        ))],
        None,
    );
    let mut session = Session::new();
    push_write(&mut session, "call-1", "fs.restore", true);

    let mut ledger = CompensatedLedger::default();
    kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("first run");
    let tool_calls = |s: &Session| {
        s.events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::ToolCall { .. }))
            .count()
    };
    let after_first = tool_calls(&session);

    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("second run");
    assert_eq!(after_first, tool_calls(&session), "no re-run");
    assert_eq!(report.undone.len(), 1, "账本命中记为已补偿");
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn irreversible_original_becomes_manual_intervention() {
    let calls = recorder();
    let kernel = kernel_with(
        vec![Arc::new(UndoTool::new(
            "fs.restore",
            SideEffect::Pure,
            Arc::clone(&calls),
        ))],
        None,
    );
    let mut session = Session::new();
    push_call(&mut session, "call-del", SideEffect::Irreversible, true);

    let mut ledger = CompensatedLedger::default();
    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("compensate ok");

    assert_eq!(report.manual.len(), 1);
    assert!(report.undone.is_empty());
    assert!(
        session
            .events()
            .iter()
            .any(|e| matches!(e, SessionEvent::ManualIntervention { .. })),
        "ManualIntervention must land in the log"
    );
    assert!(calls.lock().unwrap().is_empty(), "no tool ran");
}

#[tokio::test]
async fn failed_original_call_is_not_compensated() {
    let calls = recorder();
    let kernel = kernel_with(
        vec![Arc::new(UndoTool::new(
            "fs.restore",
            SideEffect::Pure,
            Arc::clone(&calls),
        ))],
        None,
    );
    let mut session = Session::new();
    push_write(&mut session, "call-1", "fs.restore", false);

    let mut ledger = CompensatedLedger::default();
    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("compensate ok");
    assert_eq!(report, CompensationReport::default());
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failed_undo_closes_result_and_stays_unclaimed() {
    let calls = recorder();
    let kernel = kernel_with(
        vec![Arc::new(UndoTool::with_fail(
            "fs.restore",
            SideEffect::Pure,
            Arc::clone(&calls),
            true,
        ))],
        None,
    );
    let mut session = Session::new();
    push_write(&mut session, "call-1", "fs.restore", true);

    let mut ledger = CompensatedLedger::default();
    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("compensate ok");
    assert_eq!(report.failed.len(), 1);
    assert!(
        session.events().iter().any(|e| matches!(e,
            SessionEvent::ToolResult { call_id, error: Some(msg), .. }
                if call_id.0 == "call-1-undo" && msg.contains("undo blew up"))),
        "失败的补偿也要闭合 ToolResult（纪律 5 的另一半）"
    );
    assert!(
        !ledger.contains(&CallId("call-1".into())),
        "失败不登记，可重试"
    );
}

#[tokio::test]
async fn irreversible_undo_tool_goes_through_approval() {
    // fs.restore 判 Irreversible（见 tool-fs write.rs）：无决议者 → fail-safe 拒绝
    let calls = recorder();
    let kernel = kernel_with(
        vec![Arc::new(UndoTool::new(
            "fs.restore",
            SideEffect::Irreversible,
            Arc::clone(&calls),
        ))],
        None,
    );
    let mut session = Session::new();
    push_write(&mut session, "call-1", "fs.restore", true);

    let mut ledger = CompensatedLedger::default();
    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("compensate ok");
    assert!(report.failed[0].1.contains("denied by approval"));
    assert!(calls.lock().unwrap().is_empty(), "denied undo never ran");
    assert!(
        session
            .events()
            .iter()
            .any(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: false, .. })),
        "decision recorded (纪律 9)"
    );
    assert!(!ledger.contains(&CallId("call-1".into())), "未成功不登记");

    // 决议者放行 → 执行并登记
    let calls = recorder();
    let kernel = kernel_with(
        vec![Arc::new(UndoTool::new(
            "fs.restore",
            SideEffect::Irreversible,
            Arc::clone(&calls),
        ))],
        Some(FixedResolver(ApprovalDecision::allow())),
    );
    let mut session = Session::new();
    push_write(&mut session, "call-1", "fs.restore", true);
    let mut ledger = CompensatedLedger::default();
    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("compensate ok");
    assert_eq!(report.undone.len(), 1);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn unavailable_undo_tool_is_reported_not_fatal() {
    let kernel = kernel_with(vec![], None);
    let mut session = Session::new();
    push_write(&mut session, "call-1", "fs.nope", true);

    let mut ledger = CompensatedLedger::default();
    let report = kernel
        .compensate_after(&mut session, &mut ledger, 0)
        .await
        .expect("compensate ok");
    assert!(report.failed[0].1.contains("fs.nope"));
    assert!(report.undone.is_empty());
}
