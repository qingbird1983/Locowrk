//! Agent 主循环 / Waterfall 插桩 / 洋葱模型中间件（DESIGN.md §7）。
//!
//! 阶段一模块：`kernel`（step 主循环 + LlmService / ToolSource 抽象）、
//! `middleware`（洋葱模型 trait + Next）、`chain`（链组装）。
//! 阶段四补：`waterfall`（pre-step / request / request-error 三个决策点）、
//! 中间件套件（超时/重试/熔断/审批）与 `ApprovalResolver`。
//!
//! 纪律：先写 `ToolCall` 事件再执行工具，进程崩溃后可 replay 识别悬空调用；
//! 模型适配器不特殊，只是经 `use_dyn::<dyn LlmService>` 获取的服务。

mod chain;
mod kernel;
mod middleware;

pub use chain::{ToolChain, build_chain};
pub use kernel::{
    Kernel, KernelError, LlmError, LlmRequest, LlmResponse, LlmService, StepOutcome, ToolCallReq,
    ToolSource,
};
pub use middleware::{Middleware, Next};
