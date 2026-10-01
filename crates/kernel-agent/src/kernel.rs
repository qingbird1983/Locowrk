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

mod request;

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{Context, CordisError, ScopeKey, Service};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::Value;
use session_store::{Message, Session, SessionEvent};

use crate::CompactionOutcome;
use crate::approval::{ApprovalDecision, ApprovalResolver};
use crate::middleware::Middleware;
use crate::waterfall::RetryBudget;

/// LLM 请求：直接复用 session-store 的中立投影，模型适配层再映射各家协议。
/// `tools` 是本 step 的工具目录快照（LLM.md：统一 OpenAI 风格，适配器负责
/// 转成各家格式）；`model` / `params` 可被 `agent/request` 决策环改写（§7.2）；
/// `system` 是请求级系统提示（⑪ Memory-Inject 走这里，会话日志与压缩不触碰）。
#[derive(Debug, Clone, Default)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub model: Option<String>,
    pub params: Value,
    pub system: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolCallReq {
    pub id: plugin_api::CallId,
    pub tool: String,
    pub args: Value,
    /// 发起调用的会话（step_inner 填充；LLM 解析产物初始为 None）。
    /// 审批门（⑮）据此给子 Agent 的 Irreversible 请求打标识。
    pub session_id: Option<String>,
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
    /// 全目录快照：主循环每个 step 枚举给 LLM（真实模型要看到工具才会调）。
    /// 只读枚举不失败——没有目录服务就返回空表，聊天照常。
    async fn list(&self) -> Vec<ToolSpec>;
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
    /// 中间件链（会话无关）。compensation.rs 执行 undo 调用时复用同一条链
    /// （审计/超时/熔断对补偿调用同样生效）。
    pub(crate) mw: Arc<Vec<Arc<dyn Middleware>>>,
    /// 作用域键：`Some` 时服务查询先查 scoped 层再回退 global（§3.4）。
    scope: Option<ScopeKey>,
    /// 审批决议者；缺省时 Irreversible 调用一律拒绝（fail-safe，纪律 9）。
    approval: Option<Arc<dyn ApprovalResolver>>,
    /// request-error Retry 决策的预算（§7.2）。
    retry_budget: RetryBudget,
    /// 子 Agent 快照表（⑮）：`Some` 时每步刷新 session_id → 转录前缀，
    /// `subagent_run` 工具据此 fork 子会话。
    sub: Option<Arc<crate::subagent::SubagentGate>>,
}

impl Kernel {
    pub fn new(ctx: Arc<Context>, mw: Vec<Arc<dyn Middleware>>) -> Self {
        Self {
            ctx,
            mw: Arc::new(mw),
            scope: None,
            approval: None,
            retry_budget: RetryBudget::default(),
            sub: None,
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

    /// 启用子 Agent 调度（⑮）：挂上快照表后，带会话身份的 step（step_as）
    /// 每步刷新父转录快照，`subagent_run` 工具（组装点注册）才能 fork。
    pub fn with_subagent(mut self, gate: Arc<crate::subagent::SubagentGate>) -> Self {
        self.sub = Some(gate);
        self
    }

    /// 派生子内核（⑮）：同一 Context / 中间件 / 审批门 / 重试预算，
    /// 服务查询走全新作用域——工具目录换成父视图的白名单过滤包装
    /// （子 ⊆ 父；`subagent_run` 硬剔除，单层派生）。
    pub(crate) fn child_kernel(&self, allow: Vec<String>) -> Result<Self, KernelError> {
        let parent_view = self.scoped_dyn::<dyn ToolSource>()?;
        let key = cordis_core::ScopeKey::new();
        self.ctx.provide_dyn_scoped::<dyn ToolSource>(
            &key,
            Arc::new(crate::subagent::FilterToolSource::new(parent_view, allow)),
        );
        Ok(Self {
            ctx: Arc::clone(&self.ctx),
            mw: Arc::clone(&self.mw),
            scope: Some(key),
            approval: self.approval.clone(),
            retry_budget: self.retry_budget,
            // 子不再挂快照表：单层派生在目录层已硬剔除，双保险
            sub: None,
        })
    }

    /// 作用域感知的 trait 对象服务查询（compensation.rs 解析 undo 工具也走这）。
    pub(crate) fn scoped_dyn<T: ?Sized + Service>(&self) -> Result<Arc<T>, CordisError> {
        match &self.scope {
            Some(key) => self.ctx.use_dyn_scoped::<T>(key),
            None => self.ctx.use_dyn::<T>(),
        }
    }

    pub async fn step(&self, session: &mut Session) -> Result<StepOutcome, KernelError> {
        self.step_inner(session, None).await
    }

    /// 带 会话身份 的 step（⑭）：`CallCtx.session_id`（§5.2 规划字段）自此
    /// 真正填充——组装点内置工具（schedule_create 等）据此把调度绑定到
    /// 创建它的会话。普通 `step` 不带身份（既有 17 处调用方零改动）。
    pub async fn step_as(
        &self,
        session: &mut Session,
        session_id: &str,
    ) -> Result<StepOutcome, KernelError> {
        self.step_inner(session, Some(session_id)).await
    }

    async fn step_inner(
        &self,
        session: &mut Session,
        session_id: Option<&str>,
    ) -> Result<StepOutcome, KernelError> {
        // 投影：只看游标之前的消息（回退后不含已撤销轮次，§4.2）
        let messages = session.derive_messages();

        // ⑮ 子 Agent 快照刷新（带会话身份的 step 才刷）：工具执行在
        // 本步末尾，子 Agent fork 到的前缀含本轮 user 消息与全部已完成轮。
        if let (Some(gate), Some(id)) = (&self.sub, session_id) {
            gate.note(id, session);
        }

        // waterfall: pre-step —— 插件可拒绝/改写用户消息（§7.2）
        let messages = self.waterfall_pre(messages).await?;

        // 工具目录快照随请求发给模型（真实模型要看到工具才会调；目录服务
        // 缺失时视为空表，纯聊天不要求装配 ToolSource）
        let tools = self.tool_catalog().await;

        // request-error 的 Retry 在 step 内部闭环，受重试预算约束（§7.2）
        let (resp, compacted) = self.request_with_retry(messages, tools).await?;

        // 压缩先于本轮 assistant 消息落日志（Context and Memory.md §3.7）：
        // 折叠语义见 session-store；撤销 = rewind_to 本事件。
        if let Some(c) = compacted {
            session.append(SessionEvent::ContextCompacted {
                kept_messages: c.kept_messages,
                summary: c.summary,
                from_tokens: c.from_tokens,
                to_tokens: c.to_tokens,
                strategy: c.strategy,
                ts: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
            });
        }

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

        for mut call in resp.tool_calls {
            // ⑮：调用请求携带会话身份（审批门给子 Agent 打标识用），
            // 与下方 CallCtx.session_id 同源同点
            call.session_id = session_id.map(str::to_owned);
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
                plugin_id: tool.plugin_id().map(str::to_owned),
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
            cctx.session_id = session_id.map(str::to_owned);
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

    /// 审批门：无决议者时 fail-safe 拒绝（纪律 9：不可逆调用前必须审批）。
    /// compensation.rs 对声明 Irreversible 的 undo 工具同样过这道门。
    pub(crate) async fn approve(
        &self,
        call: &ToolCallReq,
        spec: &plugin_api::ToolSpec,
    ) -> ApprovalDecision {
        match &self.approval {
            Some(resolver) => resolver.resolve(call, spec).await,
            None => ApprovalDecision::deny("no approval resolver configured"),
        }
    }
}
