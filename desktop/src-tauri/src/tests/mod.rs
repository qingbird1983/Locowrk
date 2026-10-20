//! 桌面端单元测试：runner 的日志投影循环 / demo_llm 决策规则 / 步数上限收束。
//! （plugins_root 工件发现用例在 tests_plugins_root.rs；审批门用例在
//! tests_approval.rs——400 行纪律拆出。事件出口用收集器替换 TauriSink。）
//! 批 F-17 目录化：共享假件（进程内 echo 工具 / 事件收集器 / 内核装配）留本文件，
//! 用例按面分簇见 `turn_rounds` / `demo_routing` / `convergence`。
//! S2 刀④ 加 `compact_wiring`：同一形状的**主层压缩**接线判据（含落盘同数那一半）。

mod compact_wiring;
mod convergence;
mod demo_routing;
mod turn_rounds;

use crate::demo_llm::DemoLlm;
use crate::events::{AgentEvent, EventSink};
use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{Kernel, LlmService, ToolSource};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

/// 进程内 echo 工具（不依赖插件产物）。
struct EchoTool;

fn echo_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "echo".into(),
        description: "test echo".into(),
        schema: json!({ "type": "object" }),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    })
}

#[async_trait]
impl Tool for EchoTool {
    fn spec(&self) -> &ToolSpec {
        echo_spec()
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(1, 0, 0);
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        Ok(json!({
            "echo": args.get("text").and_then(Value::as_str).unwrap_or(""),
        }))
    }
}

struct SingleToolSource(Arc<EchoTool>);

#[async_trait]
impl ToolSource for SingleToolSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        _name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        Ok(Arc::clone(&self.0) as Arc<dyn Tool>)
    }

    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        vec![echo_spec().clone()]
    }
}

/// 测试事件收集器。
#[derive(Clone, Default)]
pub(crate) struct Collector(Arc<StdMutex<Vec<AgentEvent>>>);

impl Collector {
    pub(crate) fn events(&self) -> Vec<AgentEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl EventSink for Collector {
    fn emit(&self, event: &AgentEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

pub(crate) fn kind(e: &AgentEvent) -> &'static str {
    match e {
        AgentEvent::Chunk { .. } => "chunk",
        AgentEvent::ToolCall { .. } => "tool_call",
        AgentEvent::ToolResult { .. } => "tool_result",
        AgentEvent::TurnDone { .. } => "turn_done",
        AgentEvent::ApprovalRequest(_) => "approval_request",
        AgentEvent::ContextUsage { .. } => "context_usage",
        AgentEvent::ContextCompacted { .. } => "context_compacted",
        AgentEvent::ScheduleFired { .. } => "schedule_fired",
        AgentEvent::UserMessage { .. } => "user_message",
        AgentEvent::ApprovalResolved { .. } => "approval_resolved",
        AgentEvent::Thinking { .. } => "thinking",
        AgentEvent::LlmUsage { .. } => "llm_usage",
        AgentEvent::AskUser { .. } => "ask_user",
        AgentEvent::AskResolved { .. } => "ask_resolved",
        AgentEvent::LlmRetry { .. } => "llm_retry",
        AgentEvent::TurnCapped { .. } => "turn_capped",
        AgentEvent::ToolUncertain { .. } => "tool_uncertain",
        AgentEvent::UncertainResolved { .. } => "uncertain_resolved",
        AgentEvent::ToolStatus { .. } => "tool_status",
        AgentEvent::FileChanges { .. } => "file_changes",
        AgentEvent::TurnHeader { .. } => "turn_header",
    }
}

pub(crate) fn kernel_with_tools() -> Kernel {
    kernel_with_tools_streaming(None)
}

/// 带可选 LlmStreamSink 的同型内核（⑳ 流式抑制测试用）。
pub(crate) fn kernel_with_tools_streaming(
    sink: Option<Arc<dyn kernel_agent::LlmStreamSink>>,
) -> Kernel {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(SingleToolSource(Arc::new(EchoTool))));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(DemoLlm::new()));
    if let Some(sink) = sink {
        ctx.provide_dyn::<dyn kernel_agent::LlmStreamSink>(sink);
    }
    Kernel::new(ctx, vec![])
}
