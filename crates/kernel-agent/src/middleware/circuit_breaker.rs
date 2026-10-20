//! CircuitBreaker 中间件（§7.1）：**按工具名隔离**——每个工具独立计数，
//! 连续失败达阈值后只打开该工具，冷却期过后放行一个探测请求（半开），
//! 成功闭合、失败重回打开。故障工具不连坐健康工具（2026-09 冒烟测试
//! 反馈：全局单计数器让从未失败的 fetch/fs_read 被一起封死）。
//!
//! 状态机（每工具一份）：Closed { failures } → (失败 ≥ threshold) →
//! Open { since } → (冷却期满，首个请求变探测) → HalfOpen →
//! 成功：Closed / 失败：Open。std Mutex 只护状态读写，绝不持锁跨 await。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use plugin_api::{CallCtx, ToolError};
use serde_json::Value;

use super::{Middleware, Next};

#[derive(Debug)]
enum PerTool {
    Closed { failures: u32 },
    Open { since: Instant },
    HalfOpen,
}

/// 连续失败 `threshold` 次打开（按工具名各算各的），冷却 `cooldown` 后
/// 进入半开探测。键集以注册目录为界（只对解析到的工具调用），无界增长
/// 不成立。
#[derive(Debug, Default)]
pub struct CircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    states: Mutex<HashMap<String, PerTool>>,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, cooldown: Duration) -> Self {
        Self {
            threshold: threshold.max(1),
            cooldown,
            states: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, PerTool>> {
        self.states.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// 锁内判定本次调用是否放行；Open 且冷却期满 → 转半开并放行探测。
    fn admission(&self, tool: &str) -> bool {
        let mut states = self.lock();
        let open = matches!(states.get(tool), Some(PerTool::Open { .. }));
        let expired = matches!(
            states.get(tool),
            Some(PerTool::Open { since }) if since.elapsed() >= self.cooldown
        );
        if expired {
            states.insert(tool.to_string(), PerTool::HalfOpen);
        }
        !open || expired
    }

    /// 成功即回到缺省态；直接删键，不给长尾工具留常驻记录。
    fn on_success(&self, tool: &str) {
        self.lock().remove(tool);
    }

    fn on_failure(&self, tool: &str) {
        let mut states = self.lock();
        let failures = match states.get(tool) {
            Some(PerTool::Closed { failures }) => failures + 1,
            Some(PerTool::HalfOpen) => self.threshold, // 探测失败 = 直接打开
            Some(PerTool::Open { .. }) => return,
            None => 1,
        };
        if failures >= self.threshold {
            states.insert(
                tool.to_string(),
                PerTool::Open {
                    since: Instant::now(),
                },
            );
        } else {
            states.insert(tool.to_string(), PerTool::Closed { failures });
        }
    }
}

#[async_trait]
impl Middleware for CircuitBreaker {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        let tool = next.tool_name().to_string();
        if !self.admission(&tool) {
            return Err(ToolError::Failed {
                tool,
                message: "circuit open (cooling down)".into(),
            });
        }

        match next.proceed(ctx).await {
            Ok(v) => {
                self.on_success(&tool);
                Ok(v)
            }
            Err(e) => {
                self.on_failure(&tool);
                Err(e)
            }
        }
    }
}
