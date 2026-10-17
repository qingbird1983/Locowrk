//! 阶段四验收叙事（DESIGN.md §14）：审批插件经 `agent/request` 改写参数；
//! 两 Agent（Scope）工具视图隔离。运行前置同 demo：
//! `bash scripts/build_plugins.sh`。
//!
//! 这条叙事**自带判定**（台账 T21）：末尾三条读数（danger 执行次数 / 以
//! `denied by approval` 闭合的调用数 / upper 档的结果在场）任何一条不成立就返回 Err，
//! 经 `cli::run()` 交成非零退出码。加它的理由就是这条叙事原来"跑通"过一段时间——
//! 它打印的"预期 1"与实测的 2 从来没有人比较过，而 `main` 还把退出码丢掉了。

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{Context, ScopeKey, WaterfallListener};
use kernel_agent::points;
use kernel_agent::{Kernel, StepOutcome, ToolSource};
use plugin_api::{Tool, ToolError, ToolSpec};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use session_store::{Session, SessionEvent};

use crate::approval_plugin::ApprovalPlugin;
use crate::demo::{load_and_activate, print_session};
use crate::demo4_llm::{IsolatedLlm, PolicyLlm};
use crate::inline_tools::{DangerTool, StubEchoTool};
use crate::tool_source::KernelToolSource;

const PLUGIN_ID: &str = "tool-echo";
/// 本演示只装载一档（0.2.0，含 upper / reverse）；名字讲"当前演示档"而不是
/// 具体版本号（`demo.rs` 同款，审计 F14：旧名 V13 讲的是基准重置前的 1.3.0）。
const V_ACTIVE: &str = "0.2.0";

/// 全局工具目录 = FFI 插件注册表 + 宿主进程内工具的复合。
/// 组装点职责：多个工具来源在目录层复合，主循环对此无感知。
struct CompositeToolSource {
    registry: KernelToolSource,
    danger: Arc<DangerTool>,
}

#[async_trait]
impl ToolSource for CompositeToolSource {
    async fn resolve(
        &self,
        session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        if name == "danger" {
            return Ok(Arc::clone(&self.danger) as Arc<dyn Tool>);
        }
        self.registry.resolve(session_id, name).await
    }

    async fn list(&self, session_id: Option<&str>) -> Vec<ToolSpec> {
        let mut specs = self.registry.list(session_id).await;
        specs.push(self.danger.spec().clone());
        specs
    }
}

pub async fn run(plugins_root: &Path) -> Result<(), String> {
    let ctx = Arc::new(Context::new());
    let registry = Arc::new(PluginRegistry::new());

    // 全局服务：复合工具目录（FFI 插件 + 进程内 danger）+ Agent A 的脚本 LLM
    let danger = Arc::new(DangerTool::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(CompositeToolSource {
        registry: KernelToolSource::new(RegistryToolSource::new(Arc::clone(&registry))),
        danger: Arc::clone(&danger),
    }));
    ctx.provide_dyn::<dyn kernel_agent::LlmService>(Arc::new(PolicyLlm::new()));

    // 审批插件：request 决策环 + 审批决议者，一鱼两吃
    let plugin = Arc::new(ApprovalPlugin::new());
    let listener = Arc::clone(&plugin) as Arc<dyn WaterfallListener>;
    ctx.events.on_waterfall(points::REQUEST, listener);

    // 全局加载并激活 tool-echo 0.2.0（含 upper / reverse）
    let active: Version = V_ACTIVE.parse().unwrap();
    load_and_activate(&registry, PLUGIN_ID, &active, plugins_root)?;

    // ---- 1) Agent A（§9.3 快路径：作用域层为空 → 直接走全局服务）----
    let key_a = ScopeKey::new();
    let resolver = Arc::clone(&plugin) as Arc<dyn kernel_agent::ApprovalResolver>;
    let kernel_a = Kernel::new(Arc::clone(&ctx), vec![])
        .scoped(key_a)
        .with_approval(resolver);
    let mut session_a = Session::new();
    session_a.append(SessionEvent::UserMessage {
        content: "Agent A：请执行 echo 和 danger 工具。".into(),
        parts: Vec::new(),
    });

    banner("phase A: 审批插件经 agent/request 改写参数 + 审批门");
    step(&kernel_a, &mut session_a).await?; // danger → 首次审批拒绝（未执行）
    step(&kernel_a, &mut session_a).await?; // danger → 复核后放行（执行这一次）
    step(&kernel_a, &mut session_a).await?; // echo → params.mode 被改写成 upper

    // ---- 2) Agent B（作用域层非空：同名工具解析到自己的 stub）----
    let key_b = ScopeKey::new();
    ctx.provide_dyn_scoped::<dyn ToolSource>(
        &key_b,
        Arc::new(ScopeBToolSource {
            echo: Arc::new(StubEchoTool::new()),
        }),
    );
    ctx.provide_dyn_scoped::<dyn kernel_agent::LlmService>(&key_b, Arc::new(IsolatedLlm::new()));
    let kernel_b = Kernel::new(Arc::clone(&ctx), vec![]).scoped(key_b);
    let mut session_b = Session::new();
    session_b.append(SessionEvent::UserMessage {
        content: "Agent B：请调用 echo 工具。".into(),
        parts: Vec::new(),
    });

    banner("phase B: 两 Agent 工具视图隔离（同名 echo，不同实现）");
    step(&kernel_b, &mut session_b).await?;

    // ---- 收尾 ----
    println!("\nregistry states:");
    for s in registry.list_states() {
        println!(
            "  {} active={:?} loaded={:?}",
            s.id,
            s.active.as_ref().map(Version::to_string),
            s.loaded.iter().map(Version::to_string).collect::<Vec<_>>()
        );
    }
    println!("\n=== Agent A 的会话日志 ===");
    print_session(&session_a);
    println!("\n=== Agent B 的会话日志 ===");
    print_session(&session_b);
    // ---- 自判（台账 T21）：这条叙事必须自己判定成立，不能只把数打出来给人看 ----
    // 三条读数各对应上面文案里的一句承诺；任何一条不成立就 Err（main 非零退出）。
    // 加这三条的直接起因：改之前末行写着"预期 1"而两侧实测都是 2 —— 数打出来了、
    // 预期也写了，但**没有任何东西把它俩比过**，于是一条坏掉的验收叙事可以一直"跑通"
    // （退出码 0），下一个改内核的人看见的还是那句绿话。
    let danger_calls = danger.calls();
    let denied = session_a
        .events()
        .iter()
        .filter(|e| matches!(
            e,
            SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")
        ))
        .count();
    // upper 档的实测输出形状 = `{"echo":"HELLO LOCOWRK","plugin_version":"0.2.0"}`
    // （照生产造出来的结果抄，不是照想象写：`mode` 若没被改写，这里就是小写原句）
    let upper_ok = session_a.events().iter().any(|e| {
        matches!(
            e,
            SessionEvent::ToolResult { output, .. } if output.to_string().contains("HELLO LOCOWRK")
        )
    });
    println!(
        "\ndanger 实际执行 {danger_calls} 次（预期 1）· 被拒闭合 {denied} 条（预期 1）· echo 吃到改写后的 mode = {upper_ok}（预期 true）"
    );
    if danger_calls != 1 {
        return Err(format!(
            "验收叙事不成立：danger 执行 {danger_calls} 次，预期 1 次 \
             （首次应当被审批门拦住且未执行；0 次 = 复核后也没放行，≥2 次 = 拒绝那次仍然执行了）"
        ));
    }
    if denied != 1 {
        return Err(format!(
            "验收叙事不成立：以「denied by approval」闭合的调用有 {denied} 条，预期 1 条 \
             （0 = 审批门没拦过任何东西，≥2 = 拦过头了）"
        ));
    }
    if !upper_ok {
        return Err(
            "验收叙事不成立：会话日志里没有 upper 档的结果 ⇒ 无法证明 params 被 request 决策环改写之后真的被工具吃到"
                .into(),
        );
    }
    Ok(())
}

/// 作用域 B 的工具目录：echo → 进程内 stub（遮蔽全局的插件版 echo）。
struct ScopeBToolSource {
    echo: Arc<StubEchoTool>,
}

#[async_trait]
impl ToolSource for ScopeBToolSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        if name == "echo" {
            Ok(Arc::clone(&self.echo) as Arc<dyn Tool>)
        } else {
            Err(ToolError::NotAvailable(name.to_string()))
        }
    }

    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        vec![self.echo.spec().clone()]
    }
}

async fn step(kernel: &Kernel, session: &mut Session) -> Result<(), String> {
    match kernel.step(session).await {
        Ok(StepOutcome::Continue { .. }) => Ok(()),
        Err(e) => Err(format!("kernel step: {e}")),
    }
}

fn banner(title: &str) {
    println!("\n===== {title} =====");
}
