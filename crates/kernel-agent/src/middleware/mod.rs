//! 洋葱模型中间件（DESIGN.md §7.1）。
//!
//! **实际存在的洋葱件**四件：`trace`（Tracing）、`audit`（Audit）、
//! `timeout`（Timeout）、`circuit_breaker`（CircuitBreaker）。组装次序以
//! 桌面装配点 `desktop/src-tauri/src/state/mod.rs` 为准：Tracing → Audit
//! （挂外侧，被拒也留痕）→ Timeout → CircuitBreaker → 审批门 → Tool。
//! 审批门不是洋葱件而是 Kernel 内的 InnerApproval（§7.2）——决议必须作为
//! `ApprovalDecided` 事件进会话日志，中间件没有会话句柄（见 `crate::approval`）。
//! 旧纪律里并列写的 `retry`（Retry）**从未被任何装配点注册**，已随审计修复
//! 批 C 删除（重试语义在 LLM 客户端层承载，v2.75；`WaterfallDecision::Retry`
//! 这个决策位依然活着，由 `Kernel::retry_budget` 计量）——「注释宣称有、实现
//! 里没有」正是审计报告 H2 抓出的形态，此段注释即为订正。
//!
//! 其余件都不是洋葱件，而是 `agent/request` waterfall 监听器，与本套件同住
//! 只因同属「中间件类」：`tool_exposure`（工具暴露截断，exposure 文档
//! §5/§6）、`base_system`（静态身份层 = system 字节前缀）、`memory_inject`
//! （记忆原料，Context 文档 §4/§5）、`thinking_language`（宿主注册）、
//! `instruction_files`（指令文件层）、`fragments`（尾部碎片）。注册次序 =
//! ToolExposure → BaseSystemInject → MemoryInject → 思考语言 →
//! InstructionFilesInject → FragmentInject
//! （静态段在前、只增目录殿后 = 前缀缓存友好，提示词分层设计 §6）。
//! `compact` 已降级为历史契约面（载荷类型 + token 尺子，见该模块注释），
//! 不再是监听器。`tree_context`（树目录注入）随 **S3 刀③a 移除**：方案稿 §C 把
//! 树目录段列为"随树整体退役"，而 `RequestPayload::tree_directory` 字段与
//! `TreeContextSource::plan()` **仍在**（它还管消息切片与树会话标记，生产者住在
//! `organize/` = S4 的退役面）——本线只摘注入这一档，不提前动 organize。

mod audit;
mod base_system;
mod circuit_breaker;
mod compact;
mod fragments;
mod instruction_files;
mod memory_inject;
mod timeout;
mod tool_exposure;
mod tool_retrieval;
mod trace;

#[cfg(test)]
mod tests;

pub use audit::{Audit, AuditEntry, AuditSink, JsonlAuditSink};
pub use base_system::{BaseSystemInject, base_system};
pub use circuit_breaker::CircuitBreaker;
pub use compact::{CompactionOutcome, estimate_tokens};
pub use fragments::{FragmentInject, FragmentSource, wrap_system_reminder};
pub use instruction_files::{InstructionFilesInject, InstructionSource};
pub use memory_inject::{
    LearnWeights, MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource, RecallQuery,
    VALUE_INJECT_MAX_CHARS,
};
pub use timeout::Timeout;
pub use tool_exposure::{ExposureSnapshot, GlobalEwma, ToolExposure, ToolExposureConfig};
pub use tool_retrieval::{EmbedPort, RETRIEVAL_WEIGHT, Retriever, cosine};
pub use trace::Tracing;

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, Tool, ToolError};
use serde_json::Value;

/// 类 tower 洋葱模型：`handle` 做前置/后置，`next.proceed(ctx)` 进入下一层，
/// 最内层是工具本体。`proceed` 可重复调用（重试件依赖这一点）。
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError>;
}

/// 链上"下一跳"：携带剩余中间件、终点工具与本次参数。
/// args 归 Next 所有、`proceed` 按需克隆——中间件不能原地改写下游参数；
/// 需要改写请求走 waterfall 决策点，不走中间件。
pub struct Next<'a> {
    chain: &'a [Arc<dyn Middleware>],
    idx: usize,
    tool: Arc<dyn Tool>,
    args: Value,
}

impl<'a> Next<'a> {
    pub(crate) fn new(chain: &'a [Arc<dyn Middleware>], tool: Arc<dyn Tool>, args: Value) -> Self {
        Self {
            chain,
            idx: 0,
            tool,
            args,
        }
    }

    /// 终点工具名（Tracing / Timeout / 熔断的错误消息要用）。
    pub(crate) fn tool_name(&self) -> &str {
        self.tool.spec().name.as_str()
    }

    /// 终点工具规格（Audit 记 side_effect 用）。
    pub(crate) fn tool_spec(&self) -> &plugin_api::ToolSpec {
        self.tool.spec()
    }

    /// 本次调用参数（Audit 记录"Agent 到底干了什么"；args 归 Next 所有，
    /// 只出借用，中间件仍不能改写下游参数）。
    pub(crate) fn args(&self) -> &Value {
        &self.args
    }

    pub async fn proceed(&self, ctx: &mut CallCtx) -> Result<Value, ToolError> {
        match self.chain.get(self.idx) {
            Some(mw) => {
                mw.handle(
                    ctx,
                    Next {
                        chain: self.chain,
                        idx: self.idx + 1,
                        tool: Arc::clone(&self.tool),
                        args: self.args.clone(),
                    },
                )
                .await
            }
            None => self.tool.call(ctx, self.args.clone()).await,
        }
    }
}
