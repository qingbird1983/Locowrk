//! 中间件套件单测：Tracing / Timeout / Retry / CircuitBreaker。
//! 用 mock 工具直连 `Next` 链验证行为（§7.1）。

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};

use super::{CircuitBreaker, Middleware, Next, Retry, Timeout, Tracing};

static TOOL_VERSION: semver::Version = semver::Version::new(0, 0, 1);

fn spec(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: "mock".into(),
        schema: json!({}),
        side_effect: SideEffect::Pure,
    }
}

/// 计数 mock 工具：前 `fail_first` 次调用返回 Err，之后返回 Ok。
/// `always_fail` 时永远失败；`sleep` 模拟慢调用。
struct MockTool {
    spec: ToolSpec,
    calls: AtomicU32,
    fail_first: u32,
    sleep: Duration,
}

impl MockTool {
    fn fail_first(n: u32) -> Self {
        Self {
            spec: spec("mock"),
            calls: AtomicU32::new(0),
            fail_first: n,
            sleep: Duration::ZERO,
        }
    }

    fn slow(sleep: Duration) -> Self {
        Self {
            spec: spec("mock"),
            calls: AtomicU32::new(0),
            fail_first: 0,
            sleep,
        }
    }
}

#[async_trait]
impl Tool for MockTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }

    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        tokio::time::sleep(self.sleep).await;
        let n = self.calls.fetch_add(1, Ordering::Relaxed);
        if n < self.fail_first {
            Err(ToolError::Failed {
                tool: "mock".into(),
                message: "boom".into(),
            })
        } else {
            Ok(json!({ "call": n }))
        }
    }
}

async fn run<T: Tool + 'static>(mw: Arc<dyn Middleware>, tool: T) -> Result<Value, ToolError> {
    let mut ctx = CallCtx::new();
    Next::new(std::slice::from_ref(&mw), Arc::new(tool), json!({}))
        .proceed(&mut ctx)
        .await
}

#[tokio::test]
async fn tracing_passes_result_through() {
    let out = run(Arc::new(Tracing), MockTool::fail_first(0)).await;
    assert!(out.is_ok(), "Tracing 不改变结果: {out:?}");
}

#[tokio::test]
async fn timeout_fires_on_slow_tool() {
    let mw = Arc::new(Timeout::new(Duration::from_millis(20)));
    let out = run(mw, MockTool::slow(Duration::from_millis(500))).await;
    let err = out.expect_err("慢调用必须超时");
    assert!(err.to_string().contains("timed out"), "{err}");

    // 正常快调用不受影响
    let out = run(
        Arc::new(Timeout::new(Duration::from_secs(5))),
        MockTool::fail_first(0),
    )
    .await;
    assert!(out.is_ok());
}

#[tokio::test]
async fn retry_recovers_after_transient_failure() {
    let mw = Arc::new(Retry::new(3, Duration::from_millis(1)));
    let tool = MockTool::fail_first(2); // 前两次失败，第三次成功
    let out = run(mw, tool).await;
    assert!(out.is_ok(), "第 3 次应成功: {out:?}");
}

#[tokio::test]
async fn retry_exhausts_attempts() {
    let mw = Arc::new(Retry::new(3, Duration::from_millis(1)));
    let out = run(mw, MockTool::fail_first(u32::MAX)).await;
    assert!(out.is_err(), "始终失败应耗尽尝试");
}

#[tokio::test]
async fn circuit_breaker_opens_after_threshold() {
    let mw: Arc<dyn Middleware> = Arc::new(CircuitBreaker::new(2, Duration::from_millis(30)));

    // 第 1、2 次真实到达工具并失败 → 熔断打开
    let calls = Arc::new(AtomicU32::new(0));
    for _ in 0..2 {
        let _ = run(Arc::clone(&mw), FailingTool::new(Arc::clone(&calls))).await;
    }
    assert_eq!(calls.load(Ordering::Relaxed), 2, "前两次应真实到达工具");

    // 第 3 次被熔断快速拒绝，不再到达工具
    let err = run(Arc::clone(&mw), FailingTool::new(Arc::clone(&calls)))
        .await
        .expect_err("应被熔断");
    assert!(err.to_string().contains("circuit open"), "{err}");
    assert_eq!(calls.load(Ordering::Relaxed), 2);

    // 冷却期满 → 半开放行一个探测；探测成功 → 闭合
    tokio::time::sleep(Duration::from_millis(40)).await;
    let out = run(Arc::clone(&mw), OkTool::new()).await;
    assert!(out.is_ok(), "半开探测应放行: {out:?}");

    // 闭合后继续放行
    let out = run(Arc::clone(&mw), OkTool::new()).await;
    assert!(out.is_ok());
}

struct FailingTool {
    calls: Arc<AtomicU32>,
    spec: ToolSpec,
}

impl FailingTool {
    fn new(calls: Arc<AtomicU32>) -> Self {
        Self {
            calls,
            spec: spec("mock"),
        }
    }
}

#[async_trait]
impl Tool for FailingTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }
    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(ToolError::Failed {
            tool: "mock".into(),
            message: "boom".into(),
        })
    }
}

struct OkTool {
    spec: ToolSpec,
}

impl OkTool {
    fn new() -> Self {
        Self { spec: spec("mock") }
    }
}

#[async_trait]
impl Tool for OkTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }
    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        Ok(json!("fine"))
    }
}

/// 多个共享同一熔断器实例的调用要累计计数（Arc 共享语义）。
#[tokio::test]
async fn circuit_breaker_state_is_shared_across_calls() {
    let breaker = Arc::new(CircuitBreaker::new(1, Duration::from_secs(60)));
    let calls = Arc::new(AtomicU32::new(0));
    let _ = run(
        Arc::clone(&breaker) as Arc<dyn Middleware>,
        FailingTool::new(Arc::clone(&calls)),
    )
    .await;
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    let err = run(Arc::clone(&breaker) as Arc<dyn Middleware>, OkTool::new())
        .await
        .expect_err("打开状态应快速拒绝");
    assert!(err.to_string().contains("circuit open"), "{err}");
    assert_eq!(calls.load(Ordering::Relaxed), 1, "打开期不到达工具");
}
