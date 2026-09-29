//! 桌面端单元测试：runner 的日志投影循环 / demo_llm 决策规则 /
//! plugins_root 的平台工具函数。事件出口用收集器替换 TauriSink。

use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    ApprovalDecision, ApprovalResolver, Kernel, LlmRequest, LlmService, ToolSource,
};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::{json, Value};
use session_store::{Message, Session, SessionEvent};

use crate::approval::ApprovalHub;
use crate::demo_llm::DemoLlm;
use crate::demo_tools::SendEmailTool;
use crate::events::{AgentEvent, EventSink};
use crate::plugins_root;
use crate::runner::run_turn;
use crate::tool_source::KernelToolSource;

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

    async fn list(&self) -> Vec<ToolSpec> {
        vec![echo_spec().clone()]
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
        AgentEvent::ApprovalRequest(_) => "approval_request",
    }
}

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

/// packaging.md §3.1："dll 名字由作者决定，宿主不假设文件名"——约定名
/// 缺席时扫描目录里唯一 platform 动态库；多个则拒绝猜测。
#[test]
fn find_artifact_prefers_conventional_then_single_dynlib() {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-find-artifact-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // 约定名在位 → 直接用
    let conventional = dir.join(plugins_root::dll_name("tool-x"));
    std::fs::write(&conventional, b"x").unwrap();
    assert_eq!(plugins_root::find_artifact(&dir, "tool-x"), Some(conventional));

    // 只有作者自定义名的唯一动态库 → 采用（manifest 为准的精神：不假设文件名）
    let custom = dir.join(format!("my-weird-name.{}", if cfg!(windows) { "dll" } else { "so" }));
    std::fs::remove_file(dir.join(plugins_root::dll_name("tool-x"))).unwrap();
    std::fs::write(&custom, b"x").unwrap();
    assert_eq!(plugins_root::find_artifact(&dir, "tool-x"), Some(custom.clone()));

    // 两个动态库 → 歧义拒绝
    std::fs::write(dir.join("another.dll"), b"x").unwrap();
    assert_eq!(plugins_root::find_artifact(&dir, "tool-x"), None);

    // 一个动态库 + 一个非库文件 → 仍用那个动态库
    std::fs::remove_file(dir.join("another.dll")).unwrap();
    std::fs::write(dir.join("manifest.json"), b"{}").unwrap();
    assert_eq!(plugins_root::find_artifact(&dir, "tool-x"), Some(custom));
}

// ---- 阶段六：审批门（§13.2 / 纪律 9）----

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

    let steps = run_turn(&kernel, &mut session, "s1", "发邮件".into(), &sink)
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

    // UI 投影：卡片先出现（tool_call），随后失败（tool_result）
    let kinds: Vec<&str> = sink.events().iter().map(kind).collect();
    assert_eq!(
        kinds,
        vec!["tool_call", "tool_result", "chunk", "turn_done"]
    );
}

#[tokio::test]
async fn allowed_email_executes_and_logs_decision() {
    let kernel = kernel_with_email(Arc::new(FixedResolver(ApprovalDecision::allow())));
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(&kernel, &mut session, "s1", "email 给我".into(), &sink)
        .await
        .expect("turn ok");

    assert!(session
        .events()
        .iter()
        .any(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: true, .. })));
    let AgentEvent::ToolResult { ok, output, .. } = &sink.events()[1] else {
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
                    .decide(&req.request_id, false, Some("用户拒绝".into()))
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });

    run_turn(&kernel, &mut session, "s1", "发邮件".into(), &sink)
        .await
        .expect("turn ok");

    // 前端收到审批请求事件；轮次以失败 ToolResult 收束而非卡死
    assert!(
        sink.events().iter().any(|e| kind(e) == "approval_request"),
        "approval_request 事件必须推给前端"
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
    assert!(hub.decide("call-404", true, None).is_err());
}
