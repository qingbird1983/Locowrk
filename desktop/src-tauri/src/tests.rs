//! 桌面端单元测试：runner 的日志投影循环 / demo_llm 决策规则 /
//! plugins_root 的平台工具函数。事件出口用收集器替换 TauriSink。

use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{Kernel, LlmRequest, LlmService, ToolSource};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};
use session_store::{Message, Session};

use crate::demo_llm::DemoLlm;
use crate::events::{AgentEvent, EventSink};
use crate::plugins_root;
use crate::runner::run_turn;

/// 进程内 echo 工具（不依赖插件产物）。
struct EchoTool;

fn echo_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "echo".into(),
        description: "test echo".into(),
        schema: json!({ "type": "object" }),
        side_effect: SideEffect::Pure,
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
    async fn resolve(&self, _name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        Ok(Arc::clone(&self.0) as Arc<dyn Tool>)
    }
}

/// 测试事件收集器。
#[derive(Clone, Default)]
struct Collector(Arc<StdMutex<Vec<AgentEvent>>>);

impl Collector {
    fn events(&self) -> Vec<AgentEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl EventSink for Collector {
    fn emit(&self, event: &AgentEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

fn kind(e: &AgentEvent) -> &'static str {
    match e {
        AgentEvent::Chunk { .. } => "chunk",
        AgentEvent::ToolCall { .. } => "tool_call",
        AgentEvent::ToolResult { .. } => "tool_result",
        AgentEvent::TurnDone { .. } => "turn_done",
    }
}

fn kernel_with_tools() -> Kernel {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(SingleToolSource(Arc::new(EchoTool))));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(DemoLlm::new()));
    Kernel::new(ctx, vec![])
}

#[tokio::test]
async fn tool_turn_projects_events_in_log_order() {
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(&kernel, &mut session, "s1", "echo 你好".into(), &sink)
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

#[tokio::test]
async fn plain_reply_turn_is_single_step() {
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(&kernel, &mut session, "s1", "你好".into(), &sink)
        .await
        .expect("turn ok");
    assert_eq!(steps, 1);

    let kinds: Vec<&str> = sink.events().iter().map(kind).collect();
    assert_eq!(kinds, vec!["chunk", "turn_done"]);
}

#[tokio::test]
async fn demo_llm_routes_by_content() {
    let llm = DemoLlm::new();

    let hit = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "请 echo 一下".into(),
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(hit.tool_calls.len(), 1);
    assert_eq!(hit.tool_calls[0].tool, "echo");

    let plain = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "你好".into(),
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(plain.tool_calls.is_empty());

    let after_tool = llm
        .complete(LlmRequest {
            messages: vec![Message::ToolResult {
                call_id: plugin_api::CallId("call-1".into()),
                tool: "echo".into(),
                args: json!({ "text": "hi" }),
                output: json!({ "echo": "hi" }),
                error: None,
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(after_tool.content.contains("工具返回"));
}

#[test]
fn dll_name_matches_platform() {
    let name = plugins_root::dll_name("tool-echo");
    if cfg!(windows) {
        assert_eq!(name, "tool_echo.dll");
    } else if cfg!(target_os = "macos") {
        assert_eq!(name, "libtool_echo.dylib");
    } else {
        assert_eq!(name, "libtool_echo.so");
    }
}
