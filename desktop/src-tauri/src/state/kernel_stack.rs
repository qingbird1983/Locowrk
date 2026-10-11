//! 组装点·中间件链与内核簇（DESIGN §7.1 洋葱链 + 纪律 6 审批门 + ⑮ 子 Agent 门）：
//! 超时配置 → `Vec<Arc<dyn Middleware>>` 组装 → 粘性规则库 → `ApprovalHub` →
//! `AskHub` → `SubagentGate` → `Kernel`（挂审批与子 Agent 能力）。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! 一条未重排。两条次序/共享契约：
//! 1. 洋葱序 **Tracing → Audit → Timeout → CircuitBreaker**——Audit 挂在外侧，
//!    被上游拒掉的调用也要留痕（`audit.jsonl` 只增不改）；
//! 2. `sink` 必须先于 `ApprovalHub`（审批卡经同一出口推 UI），`session_ctx` 与
//!    `permission_modes`/`permission_rules` 三者是决策表的数据源——审批、目录围栏、
//!    设置页管理命令必须读**同一实例**，各造一份就是「决策与展示不一致」的老病根
//!    （v2.145 权限模型重设计的单点化裁定）。
//!
//! `ask_hub` 与审批同款机械件（oneshot + 守夜），但它是 Pure + always 工具，
//! 不走审批门（v4.11 §3.39①）。

use std::sync::{Arc, RwLock};
use std::time::Duration;

use cordis_core::Context;
use kernel_agent::{
    Audit, CircuitBreaker, JsonlAuditSink, Kernel, Middleware, SubagentGate, Timeout, Tracing,
};

use crate::approval::ApprovalHub;
use crate::events::EventSink;
use crate::settings::SettingsStore;

pub(crate) struct Init {
    pub ctx: Arc<Context>,
    pub sink: Arc<dyn EventSink>,
    pub settings: Arc<SettingsStore>,
    pub permission_modes: Arc<crate::permission_mode::PermissionModeStore>,
    pub session_ctx: crate::workspace::SessionContextResolver,
    pub root: std::path::PathBuf,
}

/// 进 `AppState`（命令层热换与决策卡）与后续簇（subagent_run / ask_user）的句柄。
pub(crate) struct Stack {
    pub timeout_cfg: Arc<RwLock<Duration>>,
    pub permission_rules: Arc<crate::permission_rules::PermissionRuleStore>,
    pub approval: Arc<ApprovalHub>,
    pub ask_hub: Arc<crate::ask::AskHub>,
    pub gate: Arc<SubagentGate>,
    pub kernel: Arc<Kernel>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        ctx,
        sink,
        settings,
        permission_modes,
        session_ctx,
        root,
    } = init;

    // 中间件链组装次序（§7.1）：Tracing → Audit → Timeout → CircuitBreaker。Audit 挂外侧（被拒也留痕）；audit.jsonl 只增不改；S2 超时可配热换入。
    let timeout_cfg: Arc<RwLock<Duration>> = Arc::new(RwLock::new(settings.call_timeout()));
    let mw: Vec<Arc<dyn Middleware>> = vec![
        Arc::new(Tracing),
        Arc::new(Audit::new(Arc::new(JsonlAuditSink::new(
            root.join("audit.jsonl"),
        )))),
        Arc::new(Timeout::hot(Arc::clone(&timeout_cfg))),
        Arc::new(CircuitBreaker::new(5, Duration::from_secs(30))),
    ];
    // 审批门（纪律 9）：守夜超时初值取设置（v2.49 可配置，热换入；0=不限）。㉘·B：接权限模式存储（四档决策表）+ 会话→上下文（环境行/越界门围栏；㉘·C 起传全量 resolver 与插件围栏同源）
    // v2.145 项目粘性授权规则库：审批决策表与设置管理命令共读同一条 Arc
    let permission_rules = Arc::new(crate::permission_rules::PermissionRuleStore::new(&root));
    let approval = Arc::new(
        ApprovalHub::with_timeout(
            Arc::clone(&sink),
            Duration::from_secs(settings.approval_timeout_secs()),
        )
        .with_modes(Arc::clone(&permission_modes))
        .with_workspace(Arc::clone(&session_ctx))
        .with_rules(Arc::clone(&permission_rules)),
    );
    let resolver: Arc<dyn kernel_agent::ApprovalResolver> = approval.clone();
    // v4.11 §3.39① ask_user 选择卡中心：与审批同款机械件（oneshot + 守夜），
    // 超时初值取设置（0 = 不限；settings_set 热换）。Pure + always：不走
    // 审批门、常驻目录。
    let ask_hub = Arc::new(crate::ask::AskHub::new(Arc::clone(&sink)));
    ask_hub.set_timeout_secs(settings.ask_timeout_secs());
    // ⑮ 子 Agent 快照表：每 step 刷新父转录前缀；subagent_run 执行时 fork
    let gate = Arc::new(SubagentGate::default());
    let kernel = Arc::new(
        Kernel::new(ctx.clone(), mw)
            .with_approval(resolver)
            .with_subagent(Arc::clone(&gate)),
    );

    Stack {
        timeout_cfg,
        permission_rules,
        approval,
        ask_hub,
        gate,
        kernel,
    }
}
