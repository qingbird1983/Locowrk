//! Tracing 中间件（§7.1 链首）：记录每次工具调用的开始 / 结束 / 耗时。

use std::time::Instant;

use async_trait::async_trait;
use plugin_api::{CallCtx, ToolError};
use serde_json::Value;

use super::{Middleware, Next};

/// 无状态，全链共享一份即可。
#[derive(Debug, Default, Clone, Copy)]
pub struct Tracing;

#[async_trait]
impl Middleware for Tracing {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        let tool = next.tool_name();
        let started = Instant::now();
        tracing::info!(tool, call_id = ?ctx.call_id, "tool call start");

        let out = next.proceed(ctx).await;

        match &out {
            Ok(_) => tracing::info!(tool, elapsed = ?started.elapsed(), "tool call ok"),
            Err(e) => {
                tracing::warn!(tool, elapsed = ?started.elapsed(), error = %e, "tool call failed")
            }
        }
        out
    }
}
