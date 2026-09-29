//! CircuitBreaker 中间件（§7.1）：连续失败达阈值后打开，冷却期过后放行
//! 一个探测请求（半开），成功闭合、失败重回打开。
//!
//! 状态机：Closed { failures } → (失败 ≥ threshold) → Open { since }
//! → (冷却期满，首个请求变探测) → HalfOpen → 成功：Closed / 失败：Open。
//! std Mutex 只护状态读写，绝不持锁跨 await。

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use plugin_api::{CallCtx, ToolError};
use serde_json::Value;

use super::{Middleware, Next};

#[derive(Debug)]
enum State {
    Closed { failures: u32 },
    Open { since: Instant },
    HalfOpen,
}

/// 连续失败 `threshold` 次打开，冷却 `cooldown` 后进入半开探测。
#[derive(Debug)]
pub struct CircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    state: Mutex<State>,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, cooldown: Duration) -> Self {
        Self {
            threshold: threshold.max(1),
            cooldown,
            state: Mutex::new(State::Closed { failures: 0 }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// 锁内判定本次调用是否放行；Open 且冷却期满 → 转半开并放行探测。
    fn admission(&self) -> bool {
        let mut state = self.lock();
        match &*state {
            State::Open { since } if since.elapsed() >= self.cooldown => {
                *state = State::HalfOpen;
                true
            }
            State::Open { .. } => false,
            State::Closed { .. } | State::HalfOpen => true,
        }
    }

    fn on_success(&self) {
        *self.lock() = State::Closed { failures: 0 };
    }

    fn on_failure(&self) {
        let mut state = self.lock();
        match &*state {
            State::HalfOpen | State::Closed { failures: _ } => {
                let failures = match &*state {
                    State::Closed { failures } => *failures + 1,
                    _ => self.threshold, // 半开探测失败 = 直接打开
                };
                if failures >= self.threshold {
                    *state = State::Open {
                        since: Instant::now(),
                    };
                } else {
                    *state = State::Closed { failures };
                }
            }
            State::Open { .. } => {}
        }
    }
}

#[async_trait]
impl Middleware for CircuitBreaker {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        if !self.admission() {
            return Err(ToolError::Failed {
                tool: next.tool_name().to_string(),
                message: "circuit open (cooling down)".into(),
            });
        }

        match next.proceed(ctx).await {
            Ok(v) => {
                self.on_success();
                Ok(v)
            }
            Err(e) => {
                self.on_failure();
                Err(e)
            }
        }
    }
}
