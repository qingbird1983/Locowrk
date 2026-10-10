//! 中间件链组装（DESIGN.md §7.1）。
//!
//! 纪律：**实际装配序** = Tracing → Audit → Timeout → CircuitBreaker →
//! 审批门（Kernel 内 InnerApproval，非洋葱件）→ Tool；链可为空 = 直连工具。
//! 旧文本在这里并列写过 Retry，它从未被任何装配点注册、已随审计修复批 C
//! 删除（详见 `middleware/mod.rs` 头注）。

use std::sync::Arc;

use plugin_api::{CallCtx, Tool, ToolError};
use serde_json::Value;

use crate::middleware::{Middleware, Next};

/// 一次工具调用的执行链（洋葱最内层是工具本体）。
pub struct ToolChain<'a> {
    mw: &'a [Arc<dyn Middleware>],
    tool: Arc<dyn Tool>,
}

impl<'a> ToolChain<'a> {
    pub fn new(mw: &'a [Arc<dyn Middleware>], tool: Arc<dyn Tool>) -> Self {
        Self { mw, tool }
    }

    pub async fn call(&self, ctx: &mut CallCtx, args: Value) -> Result<Value, ToolError> {
        Next::new(self.mw, Arc::clone(&self.tool), args)
            .proceed(ctx)
            .await
    }
}

/// 组装便捷函数（§7.1 build_chain）：按给定顺序包中间件，末端是工具。
pub fn build_chain<'a>(mw: &'a [Arc<dyn Middleware>], tool: Arc<dyn Tool>) -> ToolChain<'a> {
    ToolChain::new(mw, tool)
}
