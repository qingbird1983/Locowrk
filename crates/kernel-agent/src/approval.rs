//! 审批（纪律 9 / §7.2）：`SideEffect::Irreversible` 工具调用前必须审批，
//! 审批决策本身也是一条事件（`ApprovalDecided`，session-store）。
//!
//! 决策必须进会话日志，而洋葱中间件没有会话句柄——所以审批门在 Kernel
//! （§7.2 的 `await_approval`），§7.1 链上的 Approval 位保留给未来
//! 独立调用路径（桌面端直接驱动工具时）。

use async_trait::async_trait;
use plugin_api::ToolSpec;

use crate::kernel::ToolCallReq;

/// 审批决议。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecision {
    pub allowed: bool,
    pub reason: Option<String>,
}

impl ApprovalDecision {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reason: None,
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.into()),
        }
    }
}

/// 审批决议者：桌面端弹窗、策略插件都实现它。
/// Kernel 对 `Irreversible` 工具调用它，并把结果写成 `ApprovalDecided` 事件。
#[async_trait]
pub trait ApprovalResolver: Send + Sync + 'static {
    async fn resolve(&self, req: &ToolCallReq, spec: &ToolSpec) -> ApprovalDecision;
}
