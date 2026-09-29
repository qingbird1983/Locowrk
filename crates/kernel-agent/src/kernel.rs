//! Agent 主循环（DESIGN.md §7.2）。
//!
//! 阶段一：最小 ReAct step——投影消息 → LLM → 先写 ToolCall 再执行 →
//! 写 ToolResult。阶段四接入：waterfall 三决策点、审批（ApprovalResolver）、
//! 重试预算。

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{Context, CordisError};
use plugin_api::{CallCtx, Tool, ToolError};
use serde_json::Value;
use session_store::{Message, Session, SessionEvent};

use crate::middleware::Middleware;

/// LLM 请求：直接复用 session-store 的中立投影，模型适配层再映射各家协议。
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct ToolCallReq {
    pub id: plugin_api::CallId,
    pub tool: String,
    pub args: Value,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCallReq>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("llm backend failed: {0}")]
    Backend(String),
}

/// 模型适配器不特殊，也只是插件：以 trait 对象服务获取
/// （插件用 provide_dyn::<dyn LlmService> 注册，§7.2）。
#[async_trait]
pub trait LlmService: Send + Sync + 'static {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;
}

/// 工具目录服务：kernel-agent 只依赖这个抽象。plugin-host 负责把
/// PluginRegistry 适配成 ToolSource 注册进 Context——主循环对 dlopen /
/// Registry 一无所知，"工具目录"也只是个服务（§7.2）。
#[async_trait]
pub trait ToolSource: Send + Sync + 'static {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// 正常完成一步（含工具执行）。
    Continue,
    /// waterfall `agent/request-error` 的 Retry 决策（阶段四接入）。
    Retry,
}

#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    #[error(transparent)]
    Service(#[from] CordisError),
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error(transparent)]
    Tool(#[from] ToolError),
}

/// 主循环。会话由调用方持有，Kernel 无会话状态（§7.2）。
pub struct Kernel {
    /// 所有服务（含 ToolSource、LlmService）从这里获取。
    ctx: Arc<Context>,
    /// 中间件链（会话无关）。
    mw: Arc<Vec<Arc<dyn Middleware>>>,
}

impl Kernel {
    pub fn new(ctx: Arc<Context>, mw: Vec<Arc<dyn Middleware>>) -> Self {
        Self {
            ctx,
            mw: Arc::new(mw),
        }
    }

    pub async fn step(&self, session: &mut Session) -> Result<StepOutcome, KernelError> {
        // 投影：只看游标之前的消息（回退后不含已撤销轮次，§4.2）
        let messages = session.derive_messages();

        let llm = self.ctx.use_dyn::<dyn LlmService>()?;
        let resp = llm.complete(LlmRequest { messages }).await?;

        session.append(SessionEvent::AssistantMessage {
            content: resp.content.clone(),
            tool_calls: resp.tool_calls.iter().map(|c| c.id.clone()).collect(),
        });

        let tools = self.ctx.use_dyn::<dyn ToolSource>()?;

        for call in resp.tool_calls {
            let tool = tools.resolve(&call.tool).await?;

            // 先写日志再执行（纪律 5）：审批与执行都发生在 ToolCall 事件之后；
            // 崩溃后 replay 能识别"执行中崩溃"的悬空调用（§7.2）。
            session.append(SessionEvent::ToolCall {
                call_id: call.id.clone(),
                tool: call.tool.clone(),
                args: call.args.clone(),
                plugin_version: tool.plugin_version().clone(),
                side_effect: tool.spec().side_effect.clone(),
            });

            // TODO(阶段四)：SideEffect::Irreversible 先走 ApprovalResolver，
            // 决策写入 ApprovalDecided 事件；拒绝则以 denied ToolResult 闭合。

            let mut cctx = CallCtx::new();
            cctx.call_id = Some(call.id.clone());
            let out = crate::chain::ToolChain::new(self.mw.as_slice(), Arc::clone(&tool))
                .call(&mut cctx, call.args.clone())
                .await;

            // 失败的调用也要闭合 ToolResult（纪律 5 的另一半：不留悬空）
            let (output, error) = match out {
                Ok(v) => (v, None),
                Err(e) => (Value::Null, Some(e.to_string())),
            };
            session.append(SessionEvent::ToolResult {
                call_id: call.id.clone(),
                output,
                error,
            });
        }

        Ok(StepOutcome::Continue)
    }
}
