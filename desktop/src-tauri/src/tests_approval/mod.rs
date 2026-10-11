//! 阶段六审批门测试（§13.2 / 纪律 9）：固定决议替身 / ApprovalHub 端到端。
//! （原在 tests.rs，400 行纪律拆出；共用收集器与 kind 从 tests 引入。）
//! v2.145 追加：项目粘性规则两级（③deny / ⑤allow）+ 子会话审批改道。
//! 批 F-14 目录化：共享假件留本文件，用例按面分簇见 `hub_basic` / `session_timeout` /
//! `project_rules` / `child_session`。

mod child_session;
mod hub_basic;
mod project_rules;
mod session_timeout;

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
use plugin_api::{CallId, Exposure, SideEffect, ToolSpec};
use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

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
