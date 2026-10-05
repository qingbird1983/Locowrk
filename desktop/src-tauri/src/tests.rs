//! 桌面端单元测试：runner 的日志投影循环 / demo_llm 决策规则 / 步数上限收束。
//! （plugins_root 工件发现用例在 tests_plugins_root.rs；审批门用例在
//! tests_approval.rs——400 行纪律拆出。事件出口用收集器替换 TauriSink。）

use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{Kernel, LlmRequest, LlmService, LlmUsage, ToolCallReq, ToolSource};
use plugin_api::CallId;
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};
use session_store::{Message, Session};

use crate::demo_llm::DemoLlm;
use crate::events::{AgentEvent, EventSink};
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
        None,
        &sink,
        None,
        None,
        1,
        None,
        None,
    )
    .await
    .expect("到顶按正常收束返回");
    assert_eq!(steps, 1);
    let kinds: Vec<&str> = sink.events().iter().map(|e| kind(e)).collect();
    assert!(kinds.contains(&"turn_done"), "正常 TurnDone 收束：{kinds:?}");
    assert!(kinds.contains(&"turn_capped"), "TurnCapped 如实告知：{kinds:?}");
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

/// 用量上报型剧本 LLM（v2.58 起真实供应商的常态：每步都带 LlmUsage）。
struct UsageLlm;

#[async_trait]
impl LlmService for UsageLlm {
    async fn complete(
        &self,
        _req: LlmRequest,
    ) -> Result<kernel_agent::LlmResponse, kernel_agent::LlmError> {
        Ok(kernel_agent::LlmResponse {
            content: "好的，收束。".into(),
            tool_calls: vec![],
            usage: Some(LlmUsage {
                input: 100,
                output: 10,
                cache_read: 90,
                cache_write: 0,
                reasoning: 0,
                total: 110,
            }),
        })
    }
}

/// M1 走查实抓（v2.58 存量缺陷回归）：报告用量的供应商下，收敛判据曾读
/// `events().last()`——LlmUsage 紧贴 assistant 落日志后 last() 恒为
/// LlmUsage，轮次永不收敛、每轮多跑一步（DeepSeek thinking 模式对以
/// assistant 结尾的请求 400 拒收，每轮尾部假 error）。回归 = 带用量的
/// 纯文本轮必须 1 步收束并发 TurnDone。
#[tokio::test]
async fn usage_reporting_provider_converges_on_final_text_step() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(SingleToolSource(Arc::new(EchoTool))));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(UsageLlm));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
        "你好".into(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
    )
    .await
    .expect("轮次必须正常收束");
    assert_eq!(
        steps, 1,
        "纯文本轮 1 步收束（不能被 LlmUsage 骗去多跑一步）"
    );
    assert!(
        sink.events()
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnDone { .. })),
        "正常收束必须发 TurnDone"
    );
}

/// 收束判据的直接补集：真有工具调用的轮次不能被误判收敛（判据改动不
/// 得放过未闭合的 tool_calls）。
#[tokio::test]
async fn usage_reporting_provider_still_loops_for_tool_calls() {
    struct ToolThenText(StdMutex<u32>);
    #[async_trait]
    impl LlmService for ToolThenText {
        async fn complete(
            &self,
            _req: LlmRequest,
        ) -> Result<kernel_agent::LlmResponse, kernel_agent::LlmError> {
            let mut n = self.0.lock().unwrap();
            let calls = if *n == 0 {
                vec![ToolCallReq {
                    id: CallId("c1".into()),
                    tool: "echo".into(),
                    args: json!({ "text": "hi" }),
                    session_id: None,
                }]
            } else {
                vec![]
            };
            *n += 1;
            drop(n);
            Ok(kernel_agent::LlmResponse {
                content: String::new(),
                tool_calls: calls,
                usage: Some(LlmUsage {
                    input: 100,
                    output: 10,
                    cache_read: 90,
                    cache_write: 0,
                    reasoning: 0,
                    total: 110,
                }),
            })
        }
    }

    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(SingleToolSource(Arc::new(EchoTool))));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ToolThenText(StdMutex::new(0))));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
        "echo 一下".into(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
    )
    .await
    .expect("轮次必须正常收束");
    assert_eq!(steps, 2, "工具轮 = 调用步 + 收束步");
    assert!(
        sink.events()
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnDone { steps: 2, .. })),
        "工具轮收束发 TurnDone(2)"
    );
}
