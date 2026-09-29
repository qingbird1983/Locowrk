//! 阶段四验收叙事（DESIGN.md §14）：审批插件经 `agent/request` 改写参数；
//! 两 Agent（Scope）工具视图隔离。运行前置同 demo：
//! `bash scripts/build_plugins.sh`。

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{Context, ScopeKey, WaterfallListener};
use kernel_agent::points;
use kernel_agent::{Kernel, StepOutcome, ToolSource};
use plugin_api::{Tool, ToolError};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use session_store::{Session, SessionEvent};

use crate::approval_plugin::ApprovalPlugin;
use crate::demo::{load_and_activate, print_session};
use crate::demo4_llm::{IsolatedLlm, PolicyLlm};
use crate::inline_tools::{DangerTool, StubEchoTool};
use crate::tool_source::KernelToolSource;

const PLUGIN_ID: &str = "tool-echo";
const V13: &str = "1.3.0";

/// 全局工具目录 = FFI 插件注册表 + 宿主进程内工具的复合。
/// 组装点职责：多个工具来源在目录层复合，主循环对此无感知。
struct CompositeToolSource {
    registry: KernelToolSource,
    danger: Arc<DangerTool>,
}

#[async_trait]
impl ToolSource for CompositeToolSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        if name == "danger" {
            return Ok(Arc::clone(&self.danger) as Arc<dyn Tool>);
        }
        self.registry.resolve(name).await
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

    // 全局加载并激活 tool-echo 1.3.0（含 upper / reverse）
    let v13: Version = V13.parse().unwrap();
    load_and_activate(&registry, PLUGIN_ID, &v13, plugins_root)?;

    // ---- 1) Agent A（§9.3 快路径：作用域层为空 → 直接走全局服务）----
    let key_a = ScopeKey::new();
    let resolver = Arc::clone(&plugin) as Arc<dyn kernel_agent::ApprovalResolver>;
    let kernel_a = Kernel::new(Arc::clone(&ctx), vec![])
        .scoped(key_a)
        .with_approval(resolver);
    let mut session_a = Session::new();
    session_a.append(SessionEvent::UserMessage {
        content: "Agent A：请执行 echo 和 danger 工具。".into(),
    });

    banner("phase A: 审批插件经 agent/request 改写参数 + 审批门");
    step(&kernel_a, &mut session_a).await?; // echo（params.mode 被改写成 upper）
    step(&kernel_a, &mut session_a).await?; // danger → 首次审批拒绝
    step(&kernel_a, &mut session_a).await?; // danger → 复核后放行

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
    println!(
        "\ndanger 工具实际执行 {} 次（预期 1：首次审批拒绝未执行，复核放行后执行）",
        danger.calls()
    );
    Ok(())
}

/// 作用域 B 的工具目录：echo → 进程内 stub（遮蔽全局的插件版 echo）。
struct ScopeBToolSource {
    echo: Arc<StubEchoTool>,
}

#[async_trait]
impl ToolSource for ScopeBToolSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        if name == "echo" {
            Ok(Arc::clone(&self.echo) as Arc<dyn Tool>)
        } else {
            Err(ToolError::NotAvailable(name.to_string()))
        }
    }
}

async fn step(kernel: &Kernel, session: &mut Session) -> Result<(), String> {
    match kernel.step(session).await {
        Ok(StepOutcome::Continue) => Ok(()),
        Err(e) => Err(format!("kernel step: {e}")),
    }
}

fn banner(title: &str) {
    println!("\n===== {title} =====");
}
