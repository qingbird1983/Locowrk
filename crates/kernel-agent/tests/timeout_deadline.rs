//! specs/04 · C2 的验收：`deadline` 的生产者接线与「取小」契约。
//!
//! 此前全仓没有 deadline 生产者（恒 None，Timeout 取小的 `Some` 分支从未被
//! 走过）。C2 把生产者落在 Timeout 中间件自身：受管工具进链时写入
//! `now + 配置时长`；自管名单早退**不写**（外层没有预算，纪律在工具内部
//! ——诚实留白）。四条用例 = 生产者两条（受管写入 / 自管留 None）+
//! 取小两个方向各一条（预算更紧 ⇒ 用预算；全局更紧 ⇒ 用全局）。
//! 变异自证（手动，判据见提交正文）：删写入 ⇒ 前两条红；删 `.min(duration)`
//! ⇒ 方向二红（方向一与生产者两条仍绿——各条只被自己的变异打中）。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use kernel_agent::{Middleware, Timeout, ToolChain};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};

static TOOL_VERSION: semver::Version = semver::Version::new(0, 0, 1);

fn spec(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: "probe".into(),
        schema: json!({}),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    }
}

/// 探针观测：(进链时刻, 看到的 deadline)；共享柄一并起名（clippy type_complexity）。
type ProbeReading = (Instant, Option<Instant>);
type SharedProbe = Arc<Mutex<Option<ProbeReading>>>;

/// 探针工具：把「进链时刻 + 看到的 deadline」抓下来。
struct DeadlineProbe {
    spec: ToolSpec,
    seen: SharedProbe,
}

impl DeadlineProbe {
    fn named(name: &str) -> (Self, SharedProbe) {
        let seen = Arc::new(Mutex::new(None));
        (
            Self {
                spec: spec(name),
                seen: Arc::clone(&seen),
            },
            seen,
        )
    }
}

#[async_trait]
impl Tool for DeadlineProbe {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }

    async fn call(&self, ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        *self.seen.lock().unwrap() = Some((Instant::now(), ctx.deadline));
        Ok(json!("probed"))
    }
}

/// 慢调用替身：睡满指定时长才返回（预算分支会在中途掐死它）。
struct SlowTool {
    spec: ToolSpec,
    sleep: Duration,
}

impl SlowTool {
    fn named(name: &str, sleep: Duration) -> Self {
        Self {
            spec: spec(name),
            sleep,
        }
    }
}

#[async_trait]
impl Tool for SlowTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }

    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        tokio::time::sleep(self.sleep).await;
        Ok(json!("done"))
    }
}

/// 预设 deadline 进链（取小两方向用预设 deadline 冒充「上游预算源」；
/// 生产者两条传 None = 走现网的真实入口形状）。
async fn run_with_deadline(
    mw: Arc<dyn Middleware>,
    tool: Arc<dyn Tool>,
    deadline: Option<Instant>,
) -> Result<Value, ToolError> {
    let mut ctx = CallCtx::new();
    ctx.deadline = deadline;
    ToolChain::new(std::slice::from_ref(&mw), tool)
        .call(&mut ctx, json!({}))
        .await
}

#[tokio::test]
async fn producer_writes_real_budget_for_governed_tools() {
    // 受管工具进链即看到 deadline ≈ now + 配置时长：它写的是中间件真要
    // enforce 的那个数（C4 长任务轮询的买主），不是另一个值。
    let global = Duration::from_millis(400);
    let (probe, seen) = DeadlineProbe::named("mock");
    let mw: Arc<dyn Middleware> = Arc::new(Timeout::new(global));
    let out = run_with_deadline(mw, Arc::new(probe), None).await;
    assert!(out.is_ok(), "探针快速返回: {out:?}");

    let (at, dl) = seen.lock().unwrap().expect("探针应被抓到");
    let dl = dl.expect("受管工具必须看到 deadline（C2 生产者）");
    let got = dl - at;
    assert!(
        got <= global && got >= global - Duration::from_millis(50),
        "deadline 应 ≈ now + 全局预算: {got:?} vs {global:?}"
    );
}

#[tokio::test]
async fn self_managed_tools_keep_deadline_none() {
    // 自管早退发生在写入之前 ⇒ None = 诚实：外层没有预算，subagent_run 的
    // timeout_secs 契约在工具内部——写个假 30s 进去反而会说谎。
    let (probe, seen) = DeadlineProbe::named("subagent_run");
    let mw: Arc<dyn Middleware> = Arc::new(Timeout::new(Duration::from_secs(5)));
    let out = run_with_deadline(mw, Arc::new(probe), None).await;
    assert!(out.is_ok(), "自管探针通过: {out:?}");
    let (_, dl) = seen.lock().unwrap().expect("探针应被抓到");
    assert!(dl.is_none(), "自管工具的 deadline 必须留 None: {dl:?}");
}

#[tokio::test]
async fn tighter_deadline_wins_over_global() {
    // 取小方向一（C2 验收）：预算更紧 ⇒ 用预算。预设 5ms / 全局 1s：
    // 取小生效 = 60ms 的替身在 ~5ms 被掐死（报错落在 500ms 内）；
    // 全局说了算 = 替身 60ms 跑完成功，本条红。
    let mw: Arc<dyn Middleware> = Arc::new(Timeout::new(Duration::from_secs(1)));
    let start = Instant::now();
    let err = run_with_deadline(
        mw,
        Arc::new(SlowTool::named("mock", Duration::from_millis(60))),
        Some(Instant::now() + Duration::from_millis(5)),
    )
    .await
    .expect_err("预算更紧时必须在预算处超时");
    assert!(err.to_string().contains("timed out after"), "{err}");
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "应在 ~5ms 掐死而不是等满全局 1s: {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn tighter_global_wins_over_deadline() {
    // 取小方向二：全局更紧 ⇒ 用全局。预设 60s / 全局 20ms：
    // 取小生效 = 替身在 ~20ms 被掐死；预设 deadline 说了算 = 替身跑完，红。
    let mw: Arc<dyn Middleware> = Arc::new(Timeout::new(Duration::from_millis(20)));
    let start = Instant::now();
    let err = run_with_deadline(
        mw,
        Arc::new(SlowTool::named("mock", Duration::from_millis(60))),
        Some(Instant::now() + Duration::from_secs(60)),
    )
    .await
    .expect_err("全局更紧时必须在全局处超时");
    assert!(err.to_string().contains("timed out after"), "{err}");
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "应在 ~20ms 掐死而不是等预设 60s: {:?}",
        start.elapsed()
    );
}
