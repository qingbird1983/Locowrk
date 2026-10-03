//! 审批门 consult 范围测试（㉘·B）：非 Pure（含 Compensable）都过决议者；
//! 无决议者时 Compensable 照旧放行、Irreversible fail-safe 拒绝（headless
//! 零变化）。目录/审批全用进程内假件，不走 FFI。

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::Context;
use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{Value, json};
use session_store::{Session, SessionEvent};

use crate::approval::{ApprovalDecision, ApprovalResolver};
use crate::kernel::{
    Kernel, LlmError, LlmRequest, LlmResponse, LlmService, ToolCallReq, ToolSource,
};

fn spec(name: &str, side: SideEffect) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: "test".into(),
        schema: json!({ "type": "object" }),
        side_effect: side,
        exposure: Default::default(),
    }
}

/// 可执行假工具：执行即回显工具名（断言"放行后真的跑了"）。
struct ProbeTool(ToolSpec);

#[async_trait]
impl Tool for ProbeTool {
    fn spec(&self) -> &ToolSpec {
        &self.0
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(0, 1, 0);
        &V
    }

    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        Ok(json!({ "ran": self.0.name }))
    }
}

struct ProbeSource(Vec<ToolSpec>);

#[async_trait]
impl ToolSource for ProbeSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        self.0
            .iter()
            .find(|s| s.name == name)
            .map(|s| Arc::new(ProbeTool(s.clone())) as Arc<dyn Tool>)
            .ok_or_else(|| ToolError::NotAvailable(name.to_string()))
    }

    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        self.0.clone()
    }
}

/// 记录 consult 过的 (tool, side_effect) 并按预设决议。
struct RecordingResolver {
    decisions: ApprovalDecision,
    consulted: Mutex<Vec<String>>,
}

#[async_trait]
impl ApprovalResolver for RecordingResolver {
    async fn resolve(
        &self,
        req: &crate::kernel::ToolCallReq,
        _spec: &ToolSpec,
    ) -> ApprovalDecision {
        self.consulted.lock().unwrap().push(req.tool.clone());
        self.decisions.clone()
    }
}

/// 剧本 LLM：第一轮发出两个工具调用，第二轮纯文本收束。
struct ScriptedLlm {
    calls: Vec<ToolCallReq>,
}

#[async_trait]
impl LlmService for ScriptedLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: String::new(),
            tool_calls: self.calls.clone(),
            usage: None,
        })
    }
}

fn call(id: &str, tool: &str) -> ToolCallReq {
    ToolCallReq {
        id: CallId(id.into()),
        tool: tool.into(),
        args: json!({}),
        session_id: Some("s1".into()),
    }
}

fn specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "fs_write",
            SideEffect::Compensable {
                undo_tool: "fs_restore".into(),
            },
        ),
        spec("shell", SideEffect::Irreversible),
        spec("fs_read", SideEffect::Pure),
    ]
}

fn turn_tools() -> Vec<ToolCallReq> {
    vec![call("c1", "fs_write"), call("c2", "shell")]
}

/// 有决议者：Compensable 也被 consult（㉘·B `询问` 档 = 写类也要问）；
/// deny 时两个调用都以 denied ToolResult 闭合、各落一条 ApprovalDecided。
#[tokio::test]
async fn resolver_is_consulted_for_compensable_too() {
    let resolver = Arc::new(RecordingResolver {
        decisions: ApprovalDecision::deny("no"),
        consulted: Mutex::new(Vec::new()),
    });
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: turn_tools(),
    }));
    let kernel = Kernel::new(ctx, vec![]).with_approval(resolver.clone() as Arc<_>);
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    let mut consulted = resolver.consulted.lock().unwrap().clone();
    consulted.sort();
    assert_eq!(consulted, vec!["fs_write".to_string(), "shell".to_string()]);

    let denied = session
        .events()
        .iter()
        .filter(|e| matches!(e, SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")))
        .count();
    assert_eq!(denied, 2, "被拒调用逐个以 denied ToolResult 闭合");
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: false, .. }))
            .count(),
        2
    );
}

/// 无决议者（headless）：Compensable 照旧放行执行、Irreversible fail-safe
/// 拒绝——headless 语义零变化（纪律 9）。
#[tokio::test]
async fn no_resolver_compensable_runs_irreversible_denied() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: turn_tools(),
    }));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    let ran_compensable = session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { output, error: None, .. }
            if output.get("ran").and_then(Value::as_str) == Some("fs_write"))
    });
    assert!(ran_compensable, "无决议者时 Compensable 照旧放行执行");
    assert!(session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { error: Some(err), .. }
            if err.contains("no approval resolver configured"))
    }));
}

/// resolve 失败（目录围栏拒掉 / 工具不存在）以失败 ToolResult 闭合该调用、
/// 步骤继续——不留悬空（悬空的 tool_calls 会让下一轮请求被供应商拒收，
/// ㉘·B 计划模式走查实抓）。
#[tokio::test]
async fn resolve_failure_closes_result_without_dangling() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    // 剧本发出两个调用：fs_write（目录里有）+ ghost（不存在 → resolve 失败）
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: vec![call("c1", "fs_write"), call("c2", "ghost")],
    }));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    kernel
        .step_as(&mut session, "s1")
        .await
        .expect("turn ok（resolve 失败不再失败整轮）");

    // ghost 以失败 ToolResult 闭合；fs_write 无决议者放行正常执行
    let ghost_closed = session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { call_id, error: Some(err), .. }
            if call_id.0 == "c2" && err.contains("not available"))
    });
    assert!(ghost_closed, "resolve 失败必须以失败 ToolResult 闭合");
    let ran = session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { call_id, output, error: None, .. }
            if call_id.0 == "c1" && output.get("ran").and_then(Value::as_str) == Some("fs_write"))
    });
    assert!(ran, "同轮其余调用照常执行");
    // 每个 tool_call 都有配对 ToolResult（供应商约束）
    for call_id in ["c1", "c2"] {
        assert!(
            session.events().iter().any(
                |e| matches!(e, SessionEvent::ToolResult { call_id: c, .. } if c.0 == call_id)
            ),
            "调用 {call_id} 必须有 ToolResult 闭合"
        );
    }
}
