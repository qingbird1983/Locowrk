//! Timeout 中间件（§7.1）：给下游调用加时间上限。
//!
//! 取配置时长与 `CallCtx::deadline` 的较小者；deadline 已过 = 立即超时。
//! 时长两态来源（S2，约束最小化清单 §3"单插件执行时间可配"）：固定值
//! （[`Timeout::new`]，测试与 CLI）或共享锁（[`Timeout::hot`]，组装点
//! 持有 `Arc<RwLock<Duration>>`，设置保存即换入——与 ToolExposureConfig
//! 同模式，本 crate 不依赖任何设置存储）。
//!
//! 自管时长名单（v2.144）：名单内工具整体不受全局预算掐断——它们自带
//! 完整的时长纪律（预算参数 + 步数上限 + 超时如实返回部分结果），全局
//! 预算只会把契约掐死：`subagent_run` 的 timeout_secs（缺省 300s/上限
//! 1800s/0 = 不限）在 30s 缺省全局预算之下永不可达（真实会话实证），
//! 内置名单即它。宿主若有同类自带预算的工具，[`Timeout::with_self_managed`]
//! 追加。

use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use plugin_api::{CallCtx, ToolError};
use serde_json::Value;

use super::{Middleware, Next};

/// 内置自管时长工具：kernel 自己的 subagent_run（时长契约见
/// `subagent::run` 的 DEFAULT_TIMEOUT_SECS / TIMEOUT_CAP_SECS）。
fn builtin_self_managed() -> BTreeSet<String> {
    BTreeSet::from(["subagent_run".to_string()])
}

#[derive(Debug, Clone)]
pub struct Timeout {
    duration: DurationSource,
    self_managed: Arc<BTreeSet<String>>,
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
            self_managed: Arc::new(builtin_self_managed()),
        }
    }

    /// 可热换时长（S2：settings.json tools 区 callTimeoutSecs）。
    pub fn hot(shared: Arc<RwLock<Duration>>) -> Self {
        Self {
            duration: DurationSource::Shared(shared),
            self_managed: Arc::new(builtin_self_managed()),
        }
    }

    /// 追加自管时长工具名（宿主侧自带预算的工具；缺省名单之外）。
    pub fn with_self_managed<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut set = (*self.self_managed).clone();
        set.extend(names.into_iter().map(Into::into));
        self.self_managed = Arc::new(set);
        self
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
        // 自管时长工具：时长纪律在工具内部（预算+步数上限+诚实部分结果），
        // 全局预算对它让位——掐在这里只会让工具的预算契约不可达。
        if self.self_managed.contains(next.tool_name()) {
            return next.proceed(ctx).await;
        }

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
