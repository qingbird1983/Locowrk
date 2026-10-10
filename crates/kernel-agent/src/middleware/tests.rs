//! 中间件套件单测：Tracing / Timeout / Retry / CircuitBreaker / Audit。
//! 用 mock 工具直连 `Next` 链验证行为（§7.1）。

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};

use super::{
    Audit, AuditEntry, AuditSink, CircuitBreaker, JsonlAuditSink, Middleware, Next, Retry, Timeout,
    Tracing,
};

static TOOL_VERSION: semver::Version = semver::Version::new(0, 0, 1);

fn spec(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: "mock".into(),
        schema: json!({}),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
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

    /// 指定工具名的慢调用 mock（自管时长名单按名字豁免，v2.144）。
    fn slow_named(name: &str, sleep: Duration) -> Self {
        Self {
            spec: spec(name),
            ..Self::slow(sleep)
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
async fn timeout_hot_duration_swaps_without_rebuild() {
    // S2 宿主可配：共享锁换入新时长，下一次调用即生效
    let shared = Arc::new(std::sync::RwLock::new(Duration::from_secs(5)));
    let mw: Arc<dyn Middleware> = Arc::new(Timeout::hot(Arc::clone(&shared)));

    let out = run(Arc::clone(&mw), MockTool::fail_first(0)).await;
    assert!(out.is_ok(), "默认时长内快调用通过");

    *shared.write().unwrap() = Duration::from_millis(20);
    let out = run(mw, MockTool::slow(Duration::from_millis(500))).await;
    let err = out.expect_err("换入短时长后慢调用必须超时");
    assert!(err.to_string().contains("timed out"), "{err}");
}

#[tokio::test]
async fn timeout_yields_to_self_managed_tools() {
    // v2.144：subagent_run 自带完整时长纪律（timeout_secs 契约 + 步数上限
    // + 超时如实返回部分结果），全局预算让位——否则其预算契约永不可达
    let mw: Arc<dyn Middleware> = Arc::new(Timeout::new(Duration::from_millis(20)));
    let out = run(
        mw,
        MockTool::slow_named("subagent_run", Duration::from_millis(120)),
    )
    .await;
    assert!(out.is_ok(), "内置自管工具不受全局预算掐断: {out:?}");

    // 宿主可追加名单：新名字同样豁免
    let mw: Arc<dyn Middleware> =
        Arc::new(Timeout::new(Duration::from_millis(20)).with_self_managed(["long_runner"]));
    let out = run(
        mw,
        MockTool::slow_named("long_runner", Duration::from_millis(120)),
    )
    .await;
    assert!(out.is_ok(), "追加名单内的工具同样豁免: {out:?}");
}

#[tokio::test]
async fn timeout_still_bounds_tools_outside_self_managed() {
    let mw: Arc<dyn Middleware> = Arc::new(Timeout::new(Duration::from_millis(20)));
    let err = run(
        mw,
        MockTool::slow_named("fs_read", Duration::from_millis(500)),
    )
    .await
    .expect_err("名单之外慢调用必须超时");
    assert!(err.to_string().contains("timed out"), "{err}");
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
        Self::named(calls, "mock")
    }
    fn named(calls: Arc<AtomicU32>, name: &str) -> Self {
        Self {
            calls,
            spec: spec(name),
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
    calls: Arc<AtomicU32>,
    spec: ToolSpec,
}

impl OkTool {
    fn new() -> Self {
        Self::named("mock", Arc::new(AtomicU32::new(0)))
    }
    fn named(name: &str, calls: Arc<AtomicU32>) -> Self {
        Self {
            calls,
            spec: spec(name),
        }
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
        self.calls.fetch_add(1, Ordering::Relaxed);
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

/// 按工具名隔离：a 熔断不连坐从未失败的 b（2026-09 冒烟测试反馈）。
#[tokio::test]
async fn circuit_breaker_is_isolated_per_tool() {
    let mw: Arc<dyn Middleware> = Arc::new(CircuitBreaker::new(2, Duration::from_secs(60)));

    // 工具 a 连续失败 2 次 → a 打开
    for _ in 0..2 {
        let _ = run(
            Arc::clone(&mw),
            FailingTool::named(Arc::new(AtomicU32::new(0)), "a"),
        )
        .await;
    }

    // 工具 b 从未被调用过 → 不受 a 连坐，真实到达工具
    let calls_b = Arc::new(AtomicU32::new(0));
    let out = run(Arc::clone(&mw), OkTool::named("b", Arc::clone(&calls_b))).await;
    assert!(out.is_ok(), "b 不应被 a 的熔断连坐: {out:?}");
    assert_eq!(calls_b.load(Ordering::Relaxed), 1, "b 应真实到达工具");

    // a 自身仍处于打开态，快速拒绝
    let err = run(
        Arc::clone(&mw),
        FailingTool::named(Arc::new(AtomicU32::new(0)), "a"),
    )
    .await
    .expect_err("a 应仍被熔断");
    assert!(err.to_string().contains("circuit open"), "{err}");
}

// ---------- Audit（plugins.md mw-audit，§7.1 洋葱模型落点）----------

/// 内存出口：捕获记录供断言。
#[derive(Default, Clone)]
struct MemSink {
    entries: Arc<std::sync::Mutex<Vec<AuditEntry>>>,
}

impl AuditSink for MemSink {
    fn record(&self, entry: AuditEntry) {
        self.entries.lock().unwrap().push(entry);
    }
}

#[tokio::test]
async fn audit_records_args_side_effect_and_failure() {
    let sink = Arc::new(MemSink::default());
    let mw: Arc<dyn Middleware> = Arc::new(Audit::new(Arc::clone(&sink) as Arc<dyn AuditSink>));

    // Compensable 规格 + 首调必败：失败那次也要留痕，且带参数与副作用等级
    let tool = MockTool {
        spec: ToolSpec {
            name: "mock".into(),
            description: "mock".into(),
            schema: json!({}),
            side_effect: SideEffect::Compensable {
                undo_tool: "undo_mock".into(),
            },
            exposure: Default::default(),
        },
        calls: AtomicU32::new(0),
        fail_first: 1,
        sleep: Duration::ZERO,
    };
    let mut ctx = CallCtx::new();
    let out = Next::new(std::slice::from_ref(&mw), Arc::new(tool), json!({"k": "v"}))
        .proceed(&mut ctx)
        .await;
    assert!(out.is_err(), "fail_first(1) 首调应失败");

    let entries = sink.entries.lock().unwrap().clone();
    assert_eq!(entries.len(), 1, "一次调用一条记录");
    let e = &entries[0];
    assert_eq!(e.tool, "mock");
    assert_eq!(e.side_effect, "compensable");
    assert_eq!(e.args["k"], "v");
    assert!(!e.ok);
    assert!(e.error.as_deref().unwrap_or("").contains("boom"));
    assert!(e.call_id.is_none(), "ctx 未设置 call_id 时不出现在记录里");
    assert!(e.ts.contains('T'), "ts 应是 RFC 3339: {}", e.ts);
}

#[tokio::test]
async fn audit_jsonl_sink_writes_parseable_lines() {
    let dir = std::env::temp_dir().join(format!("locowrk-audit-test-{}", std::process::id()));
    let file = dir.join("audit.jsonl");
    let _ = std::fs::remove_file(&file);

    let mw: Arc<dyn Middleware> = Arc::new(Audit::new(Arc::new(JsonlAuditSink::new(&file))));
    run(Arc::clone(&mw), MockTool::fail_first(0))
        .await
        .expect("成功调用不受审计影响");

    let text = std::fs::read_to_string(&file).expect("audit 文件已落盘");
    let mut lines = text.lines();
    let v: serde_json::Value =
        serde_json::from_str(lines.next().expect("至少一行")).expect("JSONL 可解析");
    assert_eq!(v["tool"], "mock");
    assert_eq!(v["side_effect"], "pure");
    assert_eq!(v["ok"].as_bool(), Some(true));
    assert!(v.get("error").is_none(), "成功记录不带 error 键");
    assert!(lines.next().is_none(), "一次调用一行");

    let _ = std::fs::remove_file(&file);
}
