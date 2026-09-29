//! Timeout 中间件（§7.1）：给下游调用加时间上限。
//!
//! 取配置时长与 `CallCtx::deadline` 的较小者；deadline 已过 = 立即超时。

use std::time::{Duration, Instant};

use async_trait::async_trait;
use plugin_api::{CallCtx, ToolError};
use serde_json::Value;

use super::{Middleware, Next};

/// 每次调用独立配置，无内部状态。
#[derive(Debug, Clone, Copy)]
pub struct Timeout {
    pub duration: Duration,
}

impl Timeout {
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

#[async_trait]
impl Middleware for Timeout {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        let budget = match ctx.deadline {
            Some(deadline) => deadline
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::ZERO)
                .min(self.duration),
            None => self.duration,
        };

        match tokio::time::timeout(budget, next.proceed(ctx)).await {
            Ok(out) => out,
            Err(_) => Err(ToolError::Failed {
                tool: next.tool_name().to_string(),
                message: format!("timed out after {:?}", budget),
            }),
        }
    }
}
