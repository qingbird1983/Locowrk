//! 洋葱模型中间件（DESIGN.md §7.1）。
//!
//! 阶段一：trait + Next 链。阶段四补齐套件：`trace`（Tracing）、`timeout`
//! （Timeout）、`retry`（Retry）、`circuit_breaker`（CircuitBreaker）。
//! 组装次序 Tracing → Timeout → Retry → CircuitBreaker → [Approval] → Tool；
//! 审批门在 Kernel（§7.2）——决策必须作为 ApprovalDecided 事件进会话日志，
//! 中间件没有会话句柄（见 `crate::approval`）。
//! `compact`（ctx-compact，plugins.md 第二批）、`memory_inject`
//! （Memory-Inject，Context 文档 §4/§5）与 `tool_exposure`（工具暴露
//! 截断，exposure 文档 §5/§6）都不是洋葱件——它们是 `agent/request`
//! waterfall 监听器，与本套件同住只因同属"中间件类"。
//! 注册次序（exposure 文档 §7 / Context 文档 §3.6 / 提示词分层设计 §6）：
//! ToolExposure 最前（先定暴露面，压缩才知道留多少空间）→
//! BaseSystemInject（静态身份层 = system 字节前缀）→ Memory-Inject
//! （记忆是原料）→ 思考语言（宿主注册）→ 树目录（append-only 殿后）。
//! 旧 ctx-compact 桌面链 v2.137 退役，headless 侧仍注册。
//! 指令文件 / 尾部碎片两件改写 `messages`（消息层面非 system），注册
//! 位置与 system 面无关，排尾部。

mod audit;
mod base_system;
mod circuit_breaker;
mod compact;
mod fragments;
mod instruction_files;
mod memory_inject;
mod retry;
mod timeout;
mod tool_exposure;
mod tool_retrieval;
mod trace;
mod tree_context;

#[cfg(test)]
mod tests;

pub use audit::{Audit, AuditEntry, AuditSink, JsonlAuditSink};
pub use base_system::{BaseSystemInject, base_system};
pub use circuit_breaker::CircuitBreaker;
pub use compact::{
    CompactConfig, CompactStrategy, CompactionOutcome, CtxCompact, estimate_tokens,
    suggested_threshold,
};
pub use fragments::{FragmentInject, FragmentSource, wrap_system_reminder};
pub use instruction_files::{InstructionFilesInject, InstructionSource};
pub use memory_inject::{MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource};
pub use retry::Retry;
pub use timeout::Timeout;
pub use tool_exposure::{ExposureSnapshot, GlobalEwma, ToolExposure, ToolExposureConfig};
pub use tool_retrieval::{EmbedPort, RETRIEVAL_WEIGHT, Retriever, cosine};
pub use trace::Tracing;
pub use tree_context::TreeDirectoryInject;

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::WaterfallDecision;
use plugin_api::{CallCtx, Tool, ToolError};
use serde::Serialize;
use serde_json::Value;

/// waterfall 监听器的 Replace 收口（to_value 失败按 fail-open 放行）——
/// ToolExposure / Memory-Inject / ctx-compact 三个 request 监听器共用。
pub(crate) fn replace_decision(req: impl Serialize) -> WaterfallDecision {
    match serde_json::to_value(&req) {
        Ok(v) => WaterfallDecision::Replace(v),
        Err(_) => WaterfallDecision::Continue,
    }
}

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
