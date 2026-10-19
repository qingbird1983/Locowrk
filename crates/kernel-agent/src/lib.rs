//! Agent 主循环 / Waterfall 插桩 / 洋葱模型中间件（DESIGN.md §7）。
//!
//! 阶段一模块：`kernel`（step 主循环 + LlmService / ToolSource 抽象）、
//! `middleware`（洋葱模型 trait + Next）、`chain`（链组装）。
//! 阶段四已补：`waterfall`（pre-step / request / request-error 三个决策点
//! 的 payload 与重试预算）、中间件套件（Tracing / Timeout / Retry /
//! CircuitBreaker）与 `approval`（ApprovalResolver 审批门）。
//!
//! 纪律：先写 `ToolCall` 事件再执行工具，进程崩溃后可 replay 识别悬空调用；
//! 模型适配器不特殊，只是经 `use_dyn::<dyn LlmService>` 获取的服务。

mod active_calls;
mod approval;
mod chain;
mod compensation;
mod kernel;
mod middleware;
mod stream;
mod subagent;
mod timeutil;
mod waterfall;

pub use active_calls::{ActiveCalls, ActiveGuard};
pub use approval::{ApprovalDecision, ApprovalResolver};
pub use chain::{ToolChain, build_chain};
pub use compensation::CompensationReport;
pub use kernel::{
    Kernel, KernelError, LlmError, LlmRequest, LlmResponse, LlmService, LlmUsage, StepOutcome,
    ToolCallReq, ToolSource, TreeContextSource, TreePlan, collapse_expanded, tail_user_start,
};
pub use middleware::{
    Audit, AuditEntry, AuditSink, BaseSystemInject, CircuitBreaker, CompactionOutcome, EmbedPort,
    ExposureSnapshot, FragmentInject, FragmentSource, GlobalEwma, InstructionFilesInject,
    InstructionSource, JsonlAuditSink, LearnWeights, MemoryEntry, MemoryInject, MemoryInjectConfig,
    MemorySource, Middleware, Next, RETRIEVAL_WEIGHT, RecallQuery, Retriever, Timeout,
    ToolExposure, ToolExposureConfig, Tracing, VALUE_INJECT_MAX_CHARS, base_system, cosine,
    estimate_tokens, wrap_system_reminder,
};
pub use stream::{LlmStreamSink, NoLlmStreamSink};
pub use subagent::{
    ChildModelFactory, ChildModelHandle, ChildModelReport, FilterToolSource, SubTranscriptSink,
    SubagentGate, SubagentTool,
};
pub use waterfall::{
    RequestErrorPayload, RequestGuard, RequestPayload, RetryBudget, append_block, points,
    take_request,
};
