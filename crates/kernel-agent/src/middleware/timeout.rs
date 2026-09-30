//! Timeout 中间件（§7.1）：给下游调用加时间上限。
//!
//! 取配置时长与 `CallCtx::deadline` 的较小者；deadline 已过 = 立即超时。
//! 时长两态来源（S2，约束最小化清单 §3"单插件执行时间可配"）：固定值
//! （[`Timeout::new`]，测试与 CLI）或共享锁（[`Timeout::hot`]，组装点
//! 持有 `Arc<RwLock<Duration>>`，设置保存即换入——与 CompactConfig 同
//! 模式，本 crate 不依赖任何设置存储）。

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use plugin_api::{CallCtx, ToolError};
use serde_json::Value;

use super::{Middleware, Next};

#[derive(Debug, Clone)]
pub struct Timeout {
    duration: DurationSource,
}

#[derive(Debug, Clone)]
enum DurationSource {
    Fixed(Duration),
    Shared(Arc<RwLock<Duration>>),
}

impl Timeout {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration: DurationSource::Fixed(duration),
        }
    }

    /// 可热换时长（S2：settings.json tools 区 callTimeoutSecs）。
    pub fn hot(shared: Arc<RwLock<Duration>>) -> Self {
        Self {
            duration: DurationSource::Shared(shared),
        }
    }

    fn current(&self) -> Duration {
        match &self.duration {
            DurationSource::Fixed(d) => *d,
            DurationSource::Shared(lock) => *lock.read().expect("timeout duration poisoned"),
        }
    }
}

#[async_trait]
impl Middleware for Timeout {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        let duration = self.current();
        let budget = match ctx.deadline {
            Some(deadline) => deadline
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::ZERO)
                .min(duration),
            None => duration,
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
