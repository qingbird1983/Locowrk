//! 宿主侧工具契约（DESIGN.md §5.2）。只活在宿主进程内，绝不进入 C ABI。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 一次工具调用的标识。事件日志（session-store）与调用上下文共用，
/// 也是补偿的幂等去重键（§6.3 / 纪律 10）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CallId(pub String);

/// 副作用等级（§5.2）：内核据此决定补偿与审批策略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SideEffect {
    /// 只读，回退免费（§6.3）。
    Pure,
    /// 有补偿工具，按 `undo_tool` 逆序补偿（§6.3）。
    Compensable { undo_tool: String },
    /// 不可逆，调用前必须审批（纪律 9）。
    Irreversible,
}

/// 工具规格：每个工具自带 side_effect，不再有 manifest 级冗余字段（§5.2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema。
    pub schema: Value,
    pub side_effect: SideEffect,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool `{0}` is not available")]
    NotAvailable(String),
    #[error("tool `{tool}` failed: {message}")]
    Failed { tool: String, message: String },
}

/// 一次工具调用的宿主侧上下文（§5.2：session_id / call_id / deadline / 取消信号）。
#[derive(Clone)]
pub struct CallCtx {
    pub session_id: Option<String>,
    pub call_id: Option<CallId>,
    pub deadline: Option<Instant>,
    cancelled: Arc<AtomicBool>,
}

impl CallCtx {
    pub fn new() -> Self {
        Self {
            session_id: None,
            call_id: None,
            deadline: None,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Default for CallCtx {
    fn default() -> Self {
        Self::new()
    }
}

/// 宿主侧工具 trait（§5.2）。plugin-host 的 FFI 适配器（阶段三）与内核测试的
/// mock 工具都实现它；`plugin_version` 供事件日志与版本回退对表。
#[async_trait::async_trait]
pub trait Tool: Send + Sync + 'static {
    fn spec(&self) -> &ToolSpec;
    fn plugin_version(&self) -> &semver::Version;
    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError>;
}

#[cfg(test)]
mod tests;
