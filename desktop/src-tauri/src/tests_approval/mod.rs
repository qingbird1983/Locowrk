//! 阶段六审批门测试（§13.2 / 纪律 9）：固定决议替身 / ApprovalHub 端到端。
//! （原在 tests.rs，400 行纪律拆出；共用收集器与 kind 从 tests 引入。）
//! v2.145 追加：项目粘性规则两级（③deny / ⑤allow）+ 子会话审批改道。
//! 批 F-14 目录化：共享假件留本文件，用例按面分簇见 `hub_basic` / `session_timeout` /
//! `project_rules` / `child_session`；批 F-24a 追加 `ctx_parse`（M-18a 解析次数计数判据）
//! 与 `m18_equiv`（M-18e② `is_outside` 对 HEAD 原文的等值矩阵）；
//! 批 Q7d 主体（v2.199）追加 `keyside`（授权规则库的键位合一走生产链路）；
//! 台账 T7（本批）追加 `hang_guard`——它不测审批语义，测的是本文件末尾那套
//! `Decider` / `bounded` 的**上界本身**（挂起的测试没有证明力，还会堵住整轮门禁）。

mod child_session;
mod ctx_parse;
mod hang_guard;
mod hub_basic;
mod keyside;
mod m18_equiv;
mod project_rules;
mod session_timeout;
mod tool_status;

use crate::approval::ApprovalHub;
use crate::demo_llm::DemoLlm;
use crate::demo_tools::SendEmailTool;
use crate::permission_mode::{PermissionMode, PermissionModeStore};
use crate::permission_rules::PermissionRuleStore;
use crate::tests::Collector;
use crate::tool_source::KernelToolSource;
use cordis_core::Context;
use kernel_agent::{ApprovalResolver, Kernel, LlmService, ToolCallReq, ToolSource};
use plugin_api::ffi::CallContext;
use plugin_api::memlayout::fnv1a64;
use plugin_api::{CallId, Exposure, SideEffect, ToolSpec};
use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// 带 send_email（Irreversible）+ 指定审批决议者的内核。
fn kernel_with_email(approval: Arc<dyn ApprovalResolver>) -> Kernel {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(
        KernelToolSource::new(RegistryToolSource::new(Arc::new(PluginRegistry::new())))
            .with_builtin(Arc::new(SendEmailTool)),
    ));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(DemoLlm::new()));
    Kernel::new(ctx, vec![]).with_approval(approval)
}

// ---- v2.145 权限模型重设计（《Permission Model Redesign (ZCode
// Alignment) v1》）：项目粘性规则两级 + 子会话审批改道。 ----

/// shell 工具 spec（Irreversible——审批卡路径的形状）。
fn shell_spec() -> ToolSpec {
    ToolSpec {
        name: "shell".into(),
        description: String::new(),
        schema: json!({}),
        side_effect: SideEffect::Irreversible,
        exposure: Exposure::default(),
    }
}

/// 测试隔离根（不碰真实数据目录）。
fn tmp_root(tag: &str) -> PathBuf {
    let dir = crate::testutil::work_dir(&format!("appr-{}", tag));
    dir
}

/// 规则库文件名（队列 Q7d 起 = **归一键**的 fnv，不是调用方喂进来的原样串）。
/// 测试里凡"手改 JSON 注入 deny"的落点一律走它而不是自己 `format!`：把
/// `fnv1a64("ws-A")` 写死 = 把改前的键位形状钉进判据，归一后必红（v2.199 实测
/// 两处），而按 `scope_key` 现算在两个平台都成立。
fn rules_file_of(workspace: &str) -> String {
    format!(
        "{:08x}.json",
        fnv1a64(crate::workspace::scope_key(Some(workspace)).as_bytes())
    )
}

/// ctx 替身：s1 / s1-sub-* 归属 ws-A（子会话改道语义的数据源），其余未归属。
fn ctx_stub() -> crate::workspace::SessionContextResolver {
    Arc::new(|sid: &str| CallContext {
        session_id: Some(sid.to_string()),
        workspace: sid.starts_with("s1").then(|| "ws-A".to_string()),
        fs_roots: Vec::new(),
    })
}

fn hub_v145(
    tag: &str,
    default_mode: PermissionMode,
) -> (
    Arc<ApprovalHub>,
    Arc<PermissionRuleStore>,
    Arc<PermissionModeStore>,
    PathBuf,
) {
    let root = tmp_root(tag);
    let rules = Arc::new(PermissionRuleStore::new(&root.join("data")));
    let modes = Arc::new(PermissionModeStore::new(
        root.join("sessions"),
        default_mode,
    ));
    let hub = Arc::new(
        ApprovalHub::with_timeout(
            Arc::new(Collector::default()),
            std::time::Duration::from_millis(80),
        )
        .with_modes(Arc::clone(&modes))
        .with_workspace(ctx_stub())
        .with_rules(Arc::clone(&rules)),
    );
    (hub, rules, modes, root)
}

fn shell_req(session: &str, command: &str) -> ToolCallReq {
    ToolCallReq {
        id: CallId("c1".into()),
        tool: "shell".into(),
        args: json!({ "command": command }),
        session_id: Some(session.into()),
    }
}

// ---- 台账 T7 · 测试侧「后台决断者」的唯一形状（挂起脆弱性收口）----
//
// 改前 9 处（`tests_permission.rs` 3 + 本目录 6）各写一份
// `loop { 有卡 → decide → break ; 否则 sleep }`，**没有上界**。于是两类挂起：
// ① 调用方 `worker.await.unwrap()` 的 3 处（keyside / project_rules 两条）——
//    卡片没出现时这里永远回不来；
// ② hub 超时给 `ZERO` / `from_secs(0)` 的 4 处——0 = 不起守夜任务，卡片没出现
//    就没人回填 oneshot，`resolve` / `run_turn` 自己永远等。
// 剩下那些 hub 有 80ms 守夜的用例（含批 F-24a 把 `ZERO` 临时改成 500ms 的那条，
// 注释里写的原因就是"会挂起而不是失败"）只是不必永等，决断者循环照样泄漏在后台。
// 挂起的测试既没有证明力，又把整轮门禁堵住。
//
// 收口 = 给等待加上界：上界内没等到卡片 ⇒ **panic 点名"问人通路没弹卡"**，
// 同一条退化一律表现为失败而不是卡住。`tests_approval/hang_guard.rs` 两条用例
// 钉的就是这个上界本身（去掉上界 → 用例耗时越过阈值报红）。

/// 等到一张审批卡片的上界。远大于正常耗时（resolve 推事件后 2ms 轮询就能看到），
/// 它存在的唯一目的 = 把"永远等不到"变成失败。
pub(crate) const DECIDER_BUDGET: Duration = Duration::from_secs(10);

/// 后台决断者 = 决策卡那一侧的测试替身。缺省「放行、不带理由、不落 scope、立即决议」，
/// 其余用 builder 说清（与 `ApprovalHub::with_*` 同风格）：
/// `Decider::new(hub).deny().reason("用户拒绝")`、`.scope("project")`、
/// `.after(Duration::from_millis(300))`（不限档那条要"晚一点才点"）。
pub(crate) struct Decider {
    hub: Arc<ApprovalHub>,
    allow: bool,
    reason: Option<String>,
    scope: Option<&'static str>,
    delay: Duration,
    budget: Duration,
}

impl Decider {
    pub(crate) fn new(hub: Arc<ApprovalHub>) -> Self {
        Self {
            hub,
            allow: true,
            reason: None,
            scope: None,
            delay: Duration::ZERO,
            budget: DECIDER_BUDGET,
        }
    }

    /// 决议 = 拒绝（决策表⑤ 卡片被点"拒绝"）。
    pub(crate) fn deny(mut self) -> Self {
        self.allow = false;
        self
    }

    /// 拒绝理由（会原样进 `ApprovalDecided`，hub_basic 那条锁的就是它）。
    pub(crate) fn reason(mut self, reason: &str) -> Self {
        self.reason = Some(reason.to_string());
        self
    }

    /// 决议范围：`"session"` / `"project"`（会话豁免与项目粘性规则的落盘面）。
    pub(crate) fn scope(mut self, scope: &'static str) -> Self {
        self.scope = Some(scope);
        self
    }

    /// 决议之前先等多久——`unlimited_timeout_keeps_waiting_for_decision` 要的就是
    /// "晚 300ms 才点，若守夜存在早已抢先拒绝"。
    pub(crate) fn after(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// 覆盖挂起上界（只有 hang_guard 那两条自测用小值，别处一律 `DECIDER_BUDGET`）。
    pub(crate) fn within(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }

    /// 起任务并交回句柄：看到卡片 → 先跑 `check`（载荷形状断言的家）→ 决议 → 结束。
    /// 轮询到 `budget` 仍没有卡片 ⇒ panic，绝不无限轮询。
    pub(crate) fn spawn(
        self,
        check: impl FnOnce(&crate::events::ApprovalRequestPayload) + Send + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let Decider {
            hub,
            allow,
            reason,
            scope,
            delay,
            budget,
        } = self;
        tokio::spawn(async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            // 2ms：改前各处写 2ms 或 5ms，取小的那个——只影响"多快看到卡片"，
            // 语义不变（正常耗时远小于 DECIDER_BUDGET）。
            let step = Duration::from_millis(2);
            let mut waited = Duration::ZERO;
            loop {
                if let Some(req) = hub.pending().first().cloned() {
                    check(&req);
                    hub.decide(&req.request_id, allow, reason, scope)
                        .expect("decision accepted");
                    return;
                }
                tokio::time::sleep(step).await;
                waited += step;
                assert!(
                    waited < budget,
                    "审批卡片在 {budget:?} 内没有出现在 pending ⇒ 问人通路挂起\
                     （决策表没走到弹卡那一步，或卡片已被守夜抢先收走）。T7 之前这里是无界轮询。"
                );
            }
        })
    }
}

/// 给"必须有人回填 oneshot 才会收束"的那一步加上界（T7 的另一半：hub 超时给 0
/// 时 `resolve` / `run_turn` 自己会永远等，光把决断者收住还不够）。
pub(crate) async fn bounded<T>(
    what: &'static str,
    budget: Duration,
    f: impl std::future::Future<Output = T>,
) -> T {
    match tokio::time::timeout(budget, f).await {
        Ok(v) => v,
        Err(_) => panic!("{what} 在 {budget:?} 内没有收束 ⇒ 审批通路挂起（没人回填 oneshot）"),
    }
}
