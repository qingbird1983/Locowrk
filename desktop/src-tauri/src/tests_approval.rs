//! 阶段六审批门测试（§13.2 / 纪律 9）：固定决议替身 / ApprovalHub 端到端。
//! （原在 tests.rs，400 行纪律拆出；共用收集器与 kind 从 tests 引入。）
//! v2.145 追加：项目粘性规则两级（③deny / ⑤allow）+ 子会话审批改道。

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    ApprovalDecision, ApprovalResolver, Kernel, LlmService, ToolCallReq, ToolSource,
};
use plugin_api::ffi::CallContext;
use plugin_api::memlayout::fnv1a64;
use plugin_api::{CallId, Exposure, SideEffect, ToolSpec};
use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::json;
use session_store::{Session, SessionEvent};

use crate::approval::ApprovalHub;
use crate::demo_llm::DemoLlm;
use crate::demo_tools::SendEmailTool;
use crate::events::AgentEvent;
use crate::permission_mode::{PermissionMode, PermissionModeStore};
use crate::permission_rules::PermissionRuleStore;
use crate::runner::run_turn;
use crate::tests::{kind, Collector};
use crate::tool_source::KernelToolSource;

/// 固定决议的审批替身：不推事件、不等待，直接返回预设决策。
struct FixedResolver(ApprovalDecision);

#[async_trait]
impl ApprovalResolver for FixedResolver {
    async fn resolve(
        &self,
        _req: &kernel_agent::ToolCallReq,
        _spec: &ToolSpec,
    ) -> ApprovalDecision {
        self.0.clone()
    }
}

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

#[tokio::test]
async fn denied_email_closes_result_without_executing() {
    let kernel = kernel_with_email(Arc::new(FixedResolver(ApprovalDecision::deny("测试拒绝"))));
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
        "发邮件".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");
    assert_eq!(steps, 2);

    // 决策进日志（纪律 9），被拒调用以失败 ToolResult 闭合、不留悬空
    let decided = session.events().iter().any(|e| {
        matches!(
            e,
            SessionEvent::ApprovalDecided { allowed: false, reason: Some(r), .. } if r == "测试拒绝"
        )
    });
    assert!(decided, "ApprovalDecided 必须写入会话日志");
    let closed = session.events().iter().any(|e| {
        matches!(
            e,
            SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")
        )
    });
    assert!(closed, "被拒调用必须以 denied 的 ToolResult 闭合");

    // UI 投影：卡片先出现（tool_call），随后决策记录条 + 失败（tool_result）
    let kinds: Vec<&str> = sink.events().iter().map(kind).collect();
    assert_eq!(
        kinds,
        vec![
            "tool_call",
            "approval_resolved",
            "tool_result",
            "chunk",
            "turn_done"
        ]
    );
}

#[tokio::test]
async fn allowed_email_executes_and_logs_decision() {
    let kernel = kernel_with_email(Arc::new(FixedResolver(ApprovalDecision::allow())));
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s1",
        "email 给我".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    assert!(session
        .events()
        .iter()
        .any(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: true, .. })));
    let events = sink.events();
    let AgentEvent::ToolResult { ok, output, .. } = events
        .iter()
        .find(|e| kind(e) == "tool_result")
        .expect("expected tool_result")
    else {
        panic!("expected tool_result")
    };
    assert!(*ok);
    assert_eq!(output["sent"], json!(true));
}

/// ApprovalHub 端到端（不含 Tauri）：请求挂起 → 后台决策 → step 醒来闭合。
#[tokio::test]
async fn approval_hub_roundtrip_through_pending_map() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::new(Arc::new(sink.clone())));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    // 后台决策者：轮询 pending 表，看到请求后拒绝（模拟用户点"拒绝"）
    let decider = Arc::clone(&hub);
    tokio::spawn(async move {
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, false, Some("用户拒绝".into()), None)
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });

    run_turn(
        &kernel,
        &mut session,
        "s1",
        "发邮件".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    // 前端收到审批请求事件；轮次以失败 ToolResult 收束而非卡死
    assert!(
        sink.events().iter().any(|e| kind(e) == "approval_request"),
        "approval_request 事件必须推给前端"
    );
    // ⑮：载荷携带会话身份（子 Agent 派生的调用据此打标识徽章）
    let payload = sink.events().iter().find_map(|e| match e {
        AgentEvent::ApprovalRequest(p) => Some(p.clone()),
        _ => None,
    });
    assert_eq!(
        payload.expect("审批载荷").session_id.as_deref(),
        Some("s1"),
        "⑮ 审批载荷带会话身份"
    );
    assert!(session.events().iter().any(|e| matches!(
        e,
        SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")
    )));
    // 决策后 pending 表必须清空（list_pending_approvals 不残留）
    assert!(hub.pending().is_empty());
}

#[tokio::test]
async fn deciding_unknown_request_is_an_error() {
    let hub = ApprovalHub::new(Arc::new(Collector::default()));
    assert!(hub.decide("call-404", true, None, None).is_err());
}

/// ㉕·对话区① §3.17「允许本次对话」：scope=session 放行后，同会话同工具
/// 的下一次调用豁免命中——不再推 approval-request；别的会话照问。
#[tokio::test]
async fn session_scope_exempts_same_tool_next_call() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::new(Arc::new(sink.clone())));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    // 第一轮：看到请求后按「允许本次对话」放行
    let decider = Arc::clone(&hub);
    tokio::spawn(async move {
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, true, None, Some("session"))
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
    run_turn(
        &kernel,
        &mut session,
        "s1",
        "发邮件".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
    )
    .await
    .expect("turn 1 ok");
    assert_eq!(
        sink.events()
            .iter()
            .filter(|e| kind(e) == "approval_request")
            .count(),
        1,
        "第一轮必须推过一次请求"
    );

    // 第二轮：豁免命中，不再问人（不新增 approval_request）
    let asked_before = sink
        .events()
        .iter()
        .filter(|e| kind(e) == "approval_request")
        .count();
    run_turn(
        &kernel,
        &mut session,
        "s1",
        "再发一封邮件".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
    )
    .await
    .expect("turn 2 ok");
    let asked_after = sink
        .events()
        .iter()
        .filter(|e| kind(e) == "approval_request")
        .count();
    assert_eq!(asked_after, asked_before, "豁免命中不得再推请求");
    // 放行照写审计：两条 ApprovalDecided（每轮一条）
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: true, .. }))
            .count(),
        2,
        "豁免放行也必须逐条落 ApprovalDecided"
    );
}

/// ㉕·对话区① §3.17 超时自动拒绝：短超时守夜到期 → fail-closed 拒绝收束，
/// ApprovalDecided 带超时理由落日志。
#[tokio::test]
async fn timeout_guard_denies_when_unanswered() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::with_timeout(
        Arc::new(sink.clone()),
        std::time::Duration::from_millis(50),
    ));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    run_turn(
        &kernel,
        &mut session,
        "s1",
        "发邮件".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok（超时按拒绝收束，不卡死）");

    assert!(hub.pending().is_empty(), "守夜必须清掉 pending 表");
    let timed_out = session.events().iter().any(|e| {
        matches!(
            e,
            SessionEvent::ApprovalDecided { allowed: false, reason: Some(r), .. }
                if r.contains("超时自动拒绝")
        )
    });
    assert!(timed_out, "超时拒绝必须带 fail-closed 理由落日志");
    assert!(
        session.events().iter().any(|e| matches!(
            e,
            SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")
        )),
        "超时拒绝的调用以失败 ToolResult 闭合"
    );
}

/// v2.49 可配置原则：`tools.approvalTimeoutSecs = 0` = 不限——不起守夜
/// 任务，审批点保持等待直到用户决策（decide 手动收束）；载荷带
/// `timeout_secs` 给决策卡倒计时。守夜机制本体由
/// `timeout_guard_denies_when_unanswered` 锁定，这里锁"不限档不超时"。
#[tokio::test]
async fn unlimited_timeout_keeps_waiting_for_decision() {
    let sink = Collector::default();
    let hub = Arc::new(ApprovalHub::with_timeout(
        Arc::new(sink.clone()),
        std::time::Duration::from_secs(0), // 0 = 不限
    ));
    let resolver: Arc<dyn ApprovalResolver> = hub.clone();
    let kernel = kernel_with_email(resolver);
    let mut session = Session::new();

    // 决策者：300ms 后才放行——若守夜存在早已抢先拒绝
    let decider = Arc::clone(&hub);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, true, None, None)
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
    run_turn(
        &kernel,
        &mut session,
        "s1",
        "发邮件".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok（不限档等待手动决策后收束）");

    // 载荷带 timeout_secs = 0（决策卡不渲染倒计时的依据）
    let events = sink.events();
    let req_ev = events
        .iter()
        .find(|e| kind(e) == "approval_request")
        .expect("请求已推");
    assert!(matches!(req_ev, AgentEvent::ApprovalRequest(p) if p.timeout_secs == 0));
    // 决策来自用户放行而非超时拒绝
    assert!(session
        .events()
        .iter()
        .any(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: true, .. })));
    assert!(!session.events().iter().any(|e| matches!(
        e,
        SessionEvent::ApprovalDecided { reason: Some(r), .. } if r.contains("超时自动拒绝")
    )));
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
    let dir = std::env::temp_dir().join(format!("locowrk-appr-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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

/// ⑤ 项目允许规则：Ask 档下命中的命令免卡直行；未命中的照常弹卡。
#[tokio::test]
async fn project_allow_rule_skips_card_in_ask_mode() {
    let (hub, rules, _modes, root) = hub_v145("allow", PermissionMode::Ask);
    rules
        .add_allow("ws-A", "shell", Some("npm test".into()))
        .unwrap();

    // 命中：直行（rule:project-allow 审计理由，不带 mode: 前缀 = 前端照常渲染记录条）
    let d = hub
        .resolve(&shell_req("s1", "npm test"), &shell_spec())
        .await;
    assert!(d.allowed);
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));

    // 未命中：弹卡（守夜 80ms 后自动拒绝）
    let d = hub
        .resolve(&shell_req("s1", "npm run build"), &shell_spec())
        .await;
    assert!(!d.allowed);
    let _ = std::fs::remove_dir_all(&root);
}

/// ③ 项目拒绝规则先于模式放行：Auto 档也拦（比 ZCode 的 yolo 先于
/// deny 更安全，有意偏离记档）。
#[tokio::test]
async fn project_deny_rule_blocks_even_in_auto_mode() {
    let (hub, rules, _modes, root) = hub_v145("deny", PermissionMode::Auto);
    rules
        .add_allow("ws-A", "shell", Some("rm -rf build".into()))
        .unwrap();
    // UI 不产 deny——手改 JSON 注入（文件面生效语义）
    let fname = format!("{:08x}.json", fnv1a64("ws-A".as_bytes()));
    let f = root.join("data").join("permissions").join(&fname);
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
    v["deny"] = json!([{ "tool": "shell", "content": "rm -rf build" }]);
    std::fs::write(&f, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let d = hub
        .resolve(&shell_req("s1", "rm -rf build"), &shell_spec())
        .await;
    assert!(!d.allowed, "deny 规则必须拦下 Auto 档");
    assert!(d.reason.unwrap_or_default().contains("项目权限规则拒绝"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 子会话审批改道：{parent}-sub-{n} 的豁免/规则/权限档全部取父根会话。
#[tokio::test]
async fn child_session_routes_to_parent_root() {
    let (hub, rules, _modes, root) = hub_v145("child", PermissionMode::Ask);
    rules
        .add_allow("ws-A", "shell", Some("node server.js".into()))
        .unwrap();

    // 豁免种在父会话上——改道前子会话 id 全新永不命中（v2.144 缺陷）
    hub.decide_seed_for_test("s1", "shell");
    let d = hub
        .resolve(&shell_req("s1-sub-1", "cargo build"), &shell_spec())
        .await;
    assert!(d.allowed, "子会话必须命中父会话豁免");

    // 规则同理：子会话发起的 shell 调用命中 ws-A 的粘性规则
    let d = hub
        .resolve(&shell_req("s1-sub-2", "node server.js"), &shell_spec())
        .await;
    assert!(d.allowed);
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 权限档继承：父会话显式 Plan，子会话非 Pure 调用按父档硬拒
/// （v2.144 缺陷 = 回落设置缺省 Ask，弹卡）。
#[tokio::test]
async fn child_session_inherits_parent_mode() {
    let (hub, _rules, modes, root) = hub_v145("childmode", PermissionMode::Ask);
    modes.set_session_mode("s1", PermissionMode::Plan).unwrap();
    let d = hub
        .resolve(&shell_req("s1-sub-1", "npm test"), &shell_spec())
        .await;
    assert!(!d.allowed, "子会话必须继承父会话 Plan 档");
    let _ = std::fs::remove_dir_all(&root);
}

/// 「本项目始终允许」落盘：scope=project 决策持久化精确命令规则（shell），
/// 第二次同命令免卡直行。
#[tokio::test]
async fn project_scope_persists_exact_command_rule() {
    let (hub, rules, _modes, root) = hub_v145("persist", PermissionMode::Ask);

    // 第一次：弹卡（后台抓 request_id 后按 project 放行）
    let decider = Arc::clone(&hub);
    let worker = tokio::spawn(async move {
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, true, None, Some("project"))
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
    let d = hub
        .resolve(&shell_req("s1", "node server.js"), &shell_spec())
        .await;
    worker.await.unwrap();
    assert!(d.allowed, "决策后 step 醒来放行");
    // 精确命令规则落盘
    assert!(
        rules.allows("ws-A", "shell", Some("node server.js")),
        "scope=project 必须持久化规则"
    );
    assert!(
        !rules.allows("ws-A", "shell", Some("node other.js")),
        "v1 粘性 = 精确命令，不同命令不得被放宽"
    );
    // 第二次同命令：免卡直行
    let d = hub
        .resolve(&shell_req("s1", "node server.js"), &shell_spec())
        .await;
    assert!(d.allowed);
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 未归属会话的 project 决策 = 跳过持久化（当次放行仍生效，ZCode 同款）。
#[tokio::test]
async fn project_scope_without_workspace_skips_persistence() {
    let (hub, rules, _modes, root) = hub_v145("nows", PermissionMode::Ask);
    let decider = Arc::clone(&hub);
    let worker = tokio::spawn(async move {
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, true, None, Some("project"))
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
    let d = hub
        .resolve(&shell_req("other-session", "node server.js"), &shell_spec())
        .await;
    worker.await.unwrap();
    assert!(d.allowed, "当次放行不受持久化缺席影响");
    assert!(!rules.allows("ws-A", "shell", Some("node server.js")));
    let _ = std::fs::remove_dir_all(&root);
}
