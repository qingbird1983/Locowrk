//! 集成测试共用 mock：工具 / 工具目录 / 脚本 LLM / 审批决议者 / 决策环。
//! 各测试文件以 `mod common;` 引入。每个测试二进制只用到其中一部分，
//! 未用项是常态，压掉 dead_code 警告。
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use cordis_core::{Context, RejectReason, WaterfallDecision, WaterfallListener};
use kernel_agent::points;
use kernel_agent::{
    ApprovalDecision, ApprovalResolver, Kernel, LlmError, LlmRequest, LlmResponse, LlmService,
    ToolCallReq, ToolSource,
};
use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};
use session_store::{Session, SessionEvent};

pub static TOOL_VERSION: semver::Version = semver::Version::new(0, 0, 1);

/// 计数 mock 工具。
pub struct MockTool {
    spec: ToolSpec,
    calls: AtomicU32,
    output: Value,
}

impl MockTool {
    pub fn new(name: &str, side_effect: SideEffect, output: Value) -> Self {
        Self {
            spec: ToolSpec {
                name: name.to_string(),
                description: "mock".into(),
                schema: json!({}),
                side_effect,
            },
            calls: AtomicU32::new(0),
            output,
        }
    }

    pub fn calls(&self) -> u32 {
        self.calls.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl Tool for MockTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }
    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.output.clone())
    }
}

/// 固定目录的 mock 工具源。
pub struct MockSource {
    tools: HashMap<String, Arc<MockTool>>,
}

impl MockSource {
    pub fn new(tools: impl IntoIterator<Item = Arc<MockTool>>) -> Self {
        Self {
            tools: tools
                .into_iter()
                .map(|t| (t.spec.name.clone(), t))
                .collect(),
        }
    }
}

#[async_trait]
impl ToolSource for MockSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        self.tools
            .get(name)
            .map(|t| Arc::clone(t) as Arc<dyn Tool>)
            .ok_or_else(|| ToolError::NotAvailable(name.to_string()))
    }

    async fn list(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|t| t.spec.clone()).collect()
    }
}

/// 按脚本弹出响应；记录收到的请求供断言。
pub struct SeqLlm {
    responses: Mutex<Vec<Result<LlmResponse, LlmError>>>,
    pub requests: Mutex<Vec<LlmRequest>>,
}

impl SeqLlm {
    pub fn new(responses: Vec<Result<LlmResponse, LlmError>>) -> Self {
        Self {
            responses: Mutex::new(responses),
            requests: Mutex::new(vec![]),
        }
    }

    pub fn call_tool(n: usize, tool: &str) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: format!("call #{n}"),
            tool_calls: vec![ToolCallReq {
                id: CallId(format!("call-{n}")),
                tool: tool.to_string(),
                args: json!({}),
            }],
        })
    }

    pub fn ok_text(text: &str) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: text.into(),
            tool_calls: vec![],
        })
    }
}

#[async_trait]
impl LlmService for SeqLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.requests.lock().unwrap().push(req);
        self.responses
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| Err(LlmError::Backend("script exhausted".into())))
    }
}

/// 恒批准。
pub struct AllowAll;

#[async_trait]
impl ApprovalResolver for AllowAll {
    async fn resolve(&self, _req: &ToolCallReq, _spec: &ToolSpec) -> ApprovalDecision {
        ApprovalDecision::allow()
    }
}

/// 恒拒绝，带固定理由。
pub struct DenyAll(pub String);

#[async_trait]
impl ApprovalResolver for DenyAll {
    async fn resolve(&self, _req: &ToolCallReq, _spec: &ToolSpec) -> ApprovalDecision {
        ApprovalDecision::deny(self.0.clone())
    }
}

/// pre-step 恒拒绝。
pub struct PreStepRejector;

#[async_trait]
impl WaterfallListener for PreStepRejector {
    async fn handle(&self, key: &str, _payload: &Value) -> WaterfallDecision {
        assert_eq!(key, points::PRE_STEP);
        WaterfallDecision::Reject(RejectReason {
            by: "policy".into(),
            reason: "blocked".into(),
        })
    }
}

/// 把 request payload 的 params.mode 改写成 upper（§14 验收：插件改写参数）。
pub struct ModeRewriter;

#[async_trait]
impl WaterfallListener for ModeRewriter {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        assert_eq!(key, points::REQUEST);
        let mut p = payload.clone();
        if let Some(obj) = p.as_object_mut() {
            let params = obj.entry("params").or_insert(json!({}));
            if let Some(pp) = params.as_object_mut() {
                pp.insert("mode".into(), json!("upper"));
            }
        }
        WaterfallDecision::Replace(p)
    }
}

/// request-error 恒 Retry（1ms）。
pub struct RetryDecider;

#[async_trait]
impl WaterfallListener for RetryDecider {
    async fn handle(&self, key: &str, _payload: &Value) -> WaterfallDecision {
        assert_eq!(key, points::REQUEST_ERROR);
        WaterfallDecision::Retry {
            delay: Duration::from_millis(1),
        }
    }
}

/// 带一条用户消息的会话。
pub fn user_session() -> Session {
    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "hi".into(),
    });
    s
}

pub fn kernel(ctx: &Arc<Context>) -> Kernel {
    Kernel::new(Arc::clone(ctx), vec![])
}
