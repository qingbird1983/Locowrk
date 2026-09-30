//! 阶段一验收：mock ReAct 闭环（全 mock LlmService，无网络）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    Kernel, LlmError, LlmRequest, LlmResponse, LlmService, Middleware, Next, StepOutcome,
    ToolCallReq, ToolSource,
};
use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};
use session_store::{Message, Session, SessionEvent};

static TOOL_VERSION: semver::Version = semver::Version::new(0, 1, 0);

/// 脚本化 LLM：见过工具结果就给最终答案，否则要求调用 `wants` 指定的工具。
struct ScriptedLlm {
    wants: &'static str,
}

#[async_trait]
impl LlmService for ScriptedLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let saw_result = req
            .messages
            .iter()
            .any(|m| matches!(m, Message::ToolResult { .. }));
        if saw_result {
            Ok(LlmResponse {
                content: "done: echo says hi".into(),
                tool_calls: vec![],
            })
        } else {
            Ok(LlmResponse {
                content: "need tool".into(),
                tool_calls: vec![ToolCallReq {
                    id: CallId("call-1".into()),
                    tool: self.wants.into(),
                    args: json!({ "text": "hi" }),
                }],
            })
        }
    }
}

struct EchoTool {
    spec: ToolSpec,
}

#[async_trait]
impl Tool for EchoTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        Ok(json!({ "echo": args["text"] }))
    }
}

struct MapSource;

#[async_trait]
impl ToolSource for MapSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        match name {
            "echo" => Ok(Arc::new(EchoTool { spec: echo_spec() })),
            other => Err(ToolError::NotAvailable(other.into())),
        }
    }

    async fn list(&self) -> Vec<ToolSpec> {
        vec![echo_spec()]
    }
}

fn echo_spec() -> ToolSpec {
    ToolSpec {
        name: "echo".into(),
        description: "echo back".into(),
        schema: json!({}),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    }
}

#[derive(Default)]
struct Recorder {
    seen: AtomicUsize,
    events: Mutex<Vec<String>>,
}

#[async_trait]
impl Middleware for Recorder {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        self.seen.fetch_add(1, Ordering::Relaxed);
        self.events.lock().unwrap().push("mw".into());
        next.proceed(ctx).await
    }
}

fn kind(e: &SessionEvent) -> &'static str {
    match e {
        SessionEvent::UserMessage { .. } => "user",
        SessionEvent::AssistantMessage { .. } => "assistant",
        SessionEvent::ToolCall { .. } => "tool_call",
        SessionEvent::ToolResult { .. } => "tool_result",
        _ => "other",
    }
}

#[tokio::test]
async fn react_loop_runs_tool_then_final_answer() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm { wants: "echo" }));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MapSource));
    let recorder = Arc::new(Recorder::default());
    let kernel = Kernel::new(
        Arc::clone(&ctx),
        vec![Arc::clone(&recorder) as Arc<dyn Middleware>],
    );

    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "say hi via tool".into(),
    });

    // 第一步：LLM 要求工具；第二步：看到工具结果后给最终答案
    assert_eq!(
        kernel.step(&mut session).await.unwrap(),
        StepOutcome::Continue
    );
    assert_eq!(
        kernel.step(&mut session).await.unwrap(),
        StepOutcome::Continue
    );

    // 事件顺序纪律：AssistantMessage → ToolCall → ToolResult（先写日志再执行）
    let kinds: Vec<&str> = session.events().iter().map(kind).collect();
    assert_eq!(
        kinds,
        vec!["user", "assistant", "tool_call", "tool_result", "assistant"]
    );

    // 中间件包住了工具执行（洋葱模型）
    assert_eq!(recorder.seen.load(Ordering::Relaxed), 1);
    assert_eq!(*recorder.events.lock().unwrap(), vec!["mw".to_string()]);

    // 投影：user + assistant(工具调用已回填) + tool_result + assistant(最终)
    let msgs = session.derive_messages();
    assert_eq!(msgs.len(), 4);
    let session_store::Message::Assistant {
        content,
        tool_calls,
    } = &msgs[1]
    else {
        panic!("expected assistant message, got {:?}", msgs[1]);
    };
    assert_eq!(content, "need tool");
    // AssistantMessage 只存 CallId，工具名与参数从 ToolCall 事件回填（§4.2）
    assert_eq!(tool_calls[0].tool, "echo");
    assert_eq!(tool_calls[0].args, json!({ "text": "hi" }));
    let session_store::Message::ToolResult {
        tool,
        output,
        error,
        ..
    } = &msgs[2]
    else {
        panic!("expected tool result, got {:?}", msgs[2]);
    };
    assert_eq!(tool, "echo");
    assert_eq!(*output, json!({ "echo": "hi" }));
    assert!(error.is_none());
    let session_store::Message::Assistant {
        content,
        tool_calls,
    } = &msgs[3]
    else {
        panic!("expected final assistant message, got {:?}", msgs[3]);
    };
    assert_eq!(content, "done: echo says hi");
    assert!(tool_calls.is_empty());
}

#[tokio::test]
async fn unavailable_tool_fails_step_without_dangling_call() {
    let ctx = Arc::new(Context::new());
    // LLM 要求一个不存在的工具
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm { wants: "ghost" }));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MapSource));
    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);

    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "call ghost".into(),
    });

    let err = kernel.step(&mut session).await.unwrap_err();
    assert!(matches!(
        err,
        kernel_agent::KernelError::Tool(ToolError::NotAvailable(_))
    ));

    // resolve 先于 ToolCall 落账（§7.2 顺序）：解析失败 = 无悬空调用
    let kinds: Vec<&str> = session.events().iter().map(kind).collect();
    assert_eq!(kinds, vec!["user", "assistant"]);
}

/// 阶段八②：请求携带工具目录快照——真实模型要看到工具才会调（LLM.md）。
/// 阶段八②：目录服务缺失时视为空表——纯聊天不要求装配 ToolSource。
struct RecordingLlm(Mutex<Vec<LlmRequest>>);

#[async_trait]
impl LlmService for RecordingLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.0.lock().unwrap().push(req);
        Ok(LlmResponse {
            content: "ok".into(),
            tool_calls: vec![],
        })
    }
}

#[tokio::test]
async fn request_carries_tool_catalog() {
    let ctx = Arc::new(Context::new());
    let llm = Arc::new(RecordingLlm(Mutex::new(vec![])));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MapSource));
    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);

    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "hello".into(),
    });
    kernel.step(&mut session).await.unwrap();

    let req = llm.0.lock().unwrap()[0].clone();
    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.tools[0].name, "echo");
    assert_eq!(req.tools[0].side_effect, SideEffect::Pure);
}

#[tokio::test]
async fn missing_tool_source_means_empty_catalog() {
    let ctx = Arc::new(Context::new());
    let llm = Arc::new(RecordingLlm(Mutex::new(vec![])));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);

    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "hello".into(),
    });
    kernel.step(&mut session).await.unwrap();

    let req = llm.0.lock().unwrap()[0].clone();
    assert!(req.tools.is_empty());
}
