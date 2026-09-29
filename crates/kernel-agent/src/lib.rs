//! Agent 主循环 / Waterfall 插桩 / 洋葱模型中间件（DESIGN.md §7）。
//!
//! 计划模块：`kernel`（step 主循环）、`middleware`（超时/重试/熔断/审批/审计）、
//! `waterfall`（pre-step / request / request-error 三个决策点）、`chain`。
//!
//! 纪律：先写 `ToolCall` 事件再执行工具，进程崩溃后可 replay 识别悬空调用。
