//! 洋葱模型中间件（DESIGN.md §7.1）。
//!
//! 阶段一：trait + Next 链。超时/重试/熔断/审批/审计具体件在阶段四。

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, Tool, ToolError};
use serde_json::Value;

/// 类 tower 洋葱模型：`handle` 做前置/后置，`next.proceed(ctx)` 进入下一层，
/// 最内层是工具本体。`proceed` 可重复调用（阶段四的重试件依赖这一点）。
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError>;
}

/// 链上"下一跳"：携带剩余中间件、终点工具与本次参数。
/// args 归 Next 所有、`proceed` 按需克隆——中间件不能原地改写下游参数；
/// 需要改写请求走 waterfall 决策点（阶段四），不走中间件。
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
