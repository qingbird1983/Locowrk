//! Agent 主循环（DESIGN.md §7.2）。
//!
//! 阶段一：最小 ReAct step——投影消息 → LLM → 先写 ToolCall 再执行 →
//! 写 ToolResult。
//! 阶段四补齐：waterfall 三决策点（pre-step / request / request-error）、
//! request-error Retry 的重试预算、Irreversible 审批门（ApprovalResolver）、
//! 作用域服务查询（两 Agent 工具视图隔离，§3.4）。
//!
//! 纪律：先写 `ToolCall` 事件再执行工具，进程崩溃后可 replay 识别悬空调用；
//! 模型适配器不特殊，只是经 `use_dyn::<dyn LlmService>` 获取的服务。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use cordis_core::{Context, CordisError, ScopeKey, Service, WaterfallDecision};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError};
use serde_json::Value;
use session_store::{Message, Session, SessionEvent};

use crate::approval::{ApprovalDecision, ApprovalResolver};
use crate::middleware::Middleware;
use crate::waterfall::{PreStepPayload, RequestErrorPayload, RequestPayload, RetryBudget, points};

/// LLM 请求：直接复用 session-store 的中立投影，模型适配层再映射各家协议。
/// `model` / `params` 可被 `agent/request` 决策环改写（§7.2）。
#[derive(Debug, Clone, Default)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
    pub model: Option<String>,
    pub params: Value,
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
    /// 正常完成一步（含工具执行与审批闭合）。
    /// （request-error 的 Retry 在 step 内部闭环受预算约束，不再对外返回。）
    Continue,
}

#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    #[error(transparent)]
    Service(#[from] CordisError),
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error("waterfall `{point}` rejected: {reason}")]
    Rejected { point: &'static str, reason: String },
    #[error("retry budget exhausted after {retries} retries / {total_delay_ms}ms (§7.2)")]
    RetryBudgetExhausted {
        retries: u32,
        total_delay_ms: u64,
        #[source]
        source: LlmError,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// 主循环。会话由调用方持有，Kernel 无会话状态（§7.2）。
pub struct Kernel {
    /// 所有服务（含 ToolSource、LlmService）从这里获取。
    ctx: Arc<Context>,
    /// 中间件链（会话无关）。
    mw: Arc<Vec<Arc<dyn Middleware>>>,
    /// 作用域键：`Some` 时服务查询先查 scoped 层再回退 global（§3.4）。
    scope: Option<ScopeKey>,
    /// 审批决议者；缺省时 Irreversible 调用一律拒绝（fail-safe，纪律 9）。
    approval: Option<Arc<dyn ApprovalResolver>>,
    /// request-error Retry 决策的预算（§7.2）。
    retry_budget: RetryBudget,
}

impl Kernel {
    pub fn new(ctx: Arc<Context>, mw: Vec<Arc<dyn Middleware>>) -> Self {
        Self {
            ctx,
            mw: Arc::new(mw),
            scope: None,
            approval: None,
            retry_budget: RetryBudget::default(),
        }
    }

    /// 让本 Kernel 成为某作用域内的 Agent：服务查询走 `use_*_scoped`
    /// （scoped 命中优先、回退 global，§3.4）——两 Agent 工具视图隔离。
    pub fn scoped(mut self, key: ScopeKey) -> Self {
        self.scope = Some(key);
        self
    }

    /// 注册审批决议者（Irreversible 工具调用前必须审批，纪律 9）。
    pub fn with_approval(mut self, resolver: Arc<dyn ApprovalResolver>) -> Self {
        self.approval = Some(resolver);
        self
    }

    /// 自定义 request-error 的重试预算。
    pub fn with_retry_budget(mut self, budget: RetryBudget) -> Self {
        self.retry_budget = budget;
        self
    }

    /// 作用域感知的 trait 对象服务查询。
    fn scoped_dyn<T: ?Sized + Service>(&self) -> Result<Arc<T>, CordisError> {
        match &self.scope {
            Some(key) => self.ctx.use_dyn_scoped::<T>(key),
            None => self.ctx.use_dyn::<T>(),
        }
    }

    pub async fn step(&self, session: &mut Session) -> Result<StepOutcome, KernelError> {
        // 投影：只看游标之前的消息（回退后不含已撤销轮次，§4.2）
        let messages = session.derive_messages();

        // waterfall: pre-step —— 插件可拒绝/改写用户消息（§7.2）
        let messages = self.waterfall_pre(messages).await?;

        // request-error 的 Retry 在 step 内部闭环，受重试预算约束（§7.2）
        let resp = self.request_with_retry(messages).await?;

        session.append(SessionEvent::AssistantMessage {
            content: resp.content.clone(),
            tool_calls: resp.tool_calls.iter().map(|c| c.id.clone()).collect(),
        });

        // 惰性解析：无工具调用的 step 不要求装配 ToolSource
        let tools = if resp.tool_calls.is_empty() {
            None
        } else {
            Some(self.scoped_dyn::<dyn ToolSource>()?)
        };

        for call in resp.tool_calls {
            let tool = tools
                .as_ref()
                .expect("非空 tool_calls 已解析目录")
                .resolve(&call.tool)
                .await?;

            // 先写日志再执行（纪律 5）：审批与执行都发生在 ToolCall 事件之后；
            // 崩溃后 replay 能识别"执行中崩溃"的悬空调用（§7.2）。
            session.append(SessionEvent::ToolCall {
                call_id: call.id.clone(),
                tool: call.tool.clone(),
                args: call.args.clone(),
                plugin_version: tool.plugin_version().clone(),
                side_effect: tool.spec().side_effect.clone(),
            });

            // 不可逆工具先审批；决策本身作为 ApprovalDecided 事件写入日志
            // （纪律 9）。被拒调用当场以 denied 的 ToolResult 闭合——replay
            // 只会把"执行中崩溃"视为悬空，被拒调用不留悬空（§7.2）。
            if matches!(tool.spec().side_effect, SideEffect::Irreversible) {
                let decision = self.approve(&call, tool.spec()).await;
                session.append(SessionEvent::ApprovalDecided {
                    call_id: call.id.clone(),
                    allowed: decision.allowed,
                    reason: decision.reason.clone(),
                });
                if !decision.allowed {
                    session.append(SessionEvent::ToolResult {
                        call_id: call.id.clone(),
                        output: Value::Null,
                        error: Some(match decision.reason {
                            Some(reason) => format!("denied by approval: {reason}"),
                            None => "denied by approval".into(),
                        }),
                    });
                    continue;
                }
            }

            // 经中间件链执行
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

    /// `agent/pre-step` 决策环：放行 → 取改写后的消息；拒绝/重试 → 报错
    /// （重试只对请求失败点有意义，§7.2）。
    async fn waterfall_pre(&self, messages: Vec<Message>) -> Result<Vec<Message>, KernelError> {
        let payload = serde_json::to_value(PreStepPayload { messages })?;
        let out = self.ctx.events.waterfall(points::PRE_STEP, payload).await;
        match out.decision {
            WaterfallDecision::Continue | WaterfallDecision::Replace(_) => {
                let parsed: PreStepPayload = serde_json::from_value(out.payload)?;
                Ok(parsed.messages)
            }
            WaterfallDecision::Reject(reason) => Err(KernelError::Rejected {
                point: points::PRE_STEP,
                reason: reason.reason,
            }),
            WaterfallDecision::Retry { .. } => Err(KernelError::Rejected {
                point: points::PRE_STEP,
                reason: "retry is meaningless before the first request".into(),
            }),
        }
    }

    /// `agent/request` 决策环：插件可改 model / params（§14 阶段四验收）。
    async fn waterfall_request(&self, messages: Vec<Message>) -> Result<LlmRequest, KernelError> {
        let payload = serde_json::to_value(RequestPayload {
            messages,
            model: None,
            // 空对象而非 null：改写环可以直接 entry/or_insert（§7.2）
            params: Value::Object(serde_json::Map::new()),
        })?;
        let out = self.ctx.events.waterfall(points::REQUEST, payload).await;
        match out.decision {
            WaterfallDecision::Continue | WaterfallDecision::Replace(_) => {
                let parsed: RequestPayload = serde_json::from_value(out.payload)?;
                Ok(LlmRequest {
                    messages: parsed.messages,
                    model: parsed.model,
                    params: parsed.params,
                })
            }
            WaterfallDecision::Reject(reason) => Err(KernelError::Rejected {
                point: points::REQUEST,
                reason: reason.reason,
            }),
            WaterfallDecision::Retry { .. } => Err(KernelError::Rejected {
                point: points::REQUEST,
                reason: "retry without a failure is meaningless".into(),
            }),
        }
    }

    /// LLM 请求 + request-error 决策环。`agent/request-error` 返回
    /// `Retry { delay }` 时在 step 内部重试（重新走 request 决策环），
    /// 累计次数 / 累计延迟超预算即失败（§7.2 重试预算）。
    async fn request_with_retry(&self, messages: Vec<Message>) -> Result<LlmResponse, KernelError> {
        let mut retries: u32 = 0;
        let mut total_delay = Duration::ZERO;
        loop {
            let req = self.waterfall_request(messages.clone()).await?;
            let llm = self.scoped_dyn::<dyn LlmService>()?;
            match llm.complete(req).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    let payload = serde_json::to_value(RequestErrorPayload {
                        error: e.to_string(),
                    })?;
                    let out = self
                        .ctx
                        .events
                        .waterfall(points::REQUEST_ERROR, payload)
                        .await;
                    match out.decision {
                        WaterfallDecision::Retry { delay } => {
                            retries += 1;
                            total_delay += delay;
                            if retries > self.retry_budget.max_retries
                                || total_delay > self.retry_budget.max_total_delay
                            {
                                return Err(KernelError::RetryBudgetExhausted {
                                    retries,
                                    total_delay_ms: total_delay.as_millis() as u64,
                                    source: e,
                                });
                            }
                            tokio::time::sleep(delay).await;
                        }
                        // Continue / Replace / Reject 一律按原错误失败（§7.2）
                        _ => return Err(e.into()),
                    }
                }
            }
        }
    }

    /// 审批门：无决议者时 fail-safe 拒绝（纪律 9：不可逆调用前必须审批）。
    async fn approve(&self, call: &ToolCallReq, spec: &plugin_api::ToolSpec) -> ApprovalDecision {
        match &self.approval {
            Some(resolver) => resolver.resolve(call, spec).await,
            None => ApprovalDecision::deny("no approval resolver configured"),
        }
    }
}
