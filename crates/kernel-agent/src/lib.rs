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

mod approval;
mod chain;
mod compensation;
mod kernel;
mod middleware;
mod stream;
mod subagent;
mod waterfall;

pub use approval::{ApprovalDecision, ApprovalResolver};
pub use chain::{ToolChain, build_chain};
pub use compensation::CompensationReport;
pub use kernel::{
    Kernel, KernelError, LlmError, LlmRequest, LlmResponse, LlmService, StepOutcome, ToolCallReq,
    ToolSource,
};
pub use middleware::{
    Audit, AuditEntry, AuditSink, CircuitBreaker, CompactConfig, CompactStrategy,
    CompactionOutcome, CtxCompact, EmbedPort, ExperienceEntry, ExperienceSource, ExposureSnapshot,
    GlobalEwma, JsonlAuditSink, MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource,
    Middleware, Next, RETRIEVAL_WEIGHT, Retriever, Retry, Timeout, ToolExposure,
    ToolExposureConfig, Tracing, cosine, estimate_tokens, suggested_threshold,
};
pub use stream::{LlmStreamSink, NoLlmStreamSink};
pub use subagent::{FilterToolSource, SubTranscriptSink, SubagentGate, SubagentTool};
pub use waterfall::{RequestErrorPayload, RequestPayload, RetryBudget, points};
