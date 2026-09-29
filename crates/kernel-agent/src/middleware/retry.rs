//! Retry 中间件（§7.1）：失败后按固定间隔重试。
//!
//! `Next::proceed` 可重复调用（阶段一已保证）；这里不做错误分类——
//! 对超时/熔断的区分由链上位次决定（Retry 在 CircuitBreaker 外层）。

use std::time::Duration;

use async_trait::async_trait;
use plugin_api::{CallCtx, ToolError};
use serde_json::Value;

use super::{Middleware, Next};

/// `max_attempts` 含首次调用（须 ≥ 1）；失败后间隔 `delay` 再试。
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    pub max_attempts: u32,
    pub delay: Duration,
}

impl Retry {
    /// `max_attempts == 0` 视为 1（只试一次，等于没有重试）。
    pub fn new(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            delay,
        }
    }
}

#[async_trait]
impl Middleware for Retry {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        let mut attempt = 1u32;
        loop {
            match next.proceed(ctx).await {
                Ok(v) => return Ok(v),
                Err(e) if attempt < self.max_attempts => {
                    tracing::warn!(
                        tool = next.tool_name(),
                        attempt,
                        error = %e,
                        "tool call failed, retrying"
                    );
                    tokio::time::sleep(self.delay).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
