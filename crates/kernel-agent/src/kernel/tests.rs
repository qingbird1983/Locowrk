//! 审批门 consult 范围测试（㉘·B）：非 Pure（含 Compensable）都过决议者；
//! 无决议者时 Compensable 照旧放行、Irreversible fail-safe 拒绝（headless
//! 零变化）。㉘·C：决议者在场 = Pure 也过决策表（隐私门要能拦住只读工具
//! 读凭据），headless 的 Pure 照旧静默放行。目录/审批全用进程内假件，不走 FFI。

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::{Context, WaterfallDecision, WaterfallListener};
use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{Value, json};
use session_store::{Session, SessionEvent};

use crate::approval::{ApprovalDecision, ApprovalResolver};
use crate::kernel::{
    Kernel, LlmError, LlmRequest, LlmResponse, LlmService, ToolCallReq, ToolSource,
};
use crate::waterfall::points;
use crate::{
    GlobalEwma, MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource, ToolExposure,
    ToolExposureConfig,
};

fn spec(name: &str, side: SideEffect) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: "test".into(),
        schema: json!({ "type": "object" }),
        side_effect: side,
        exposure: Default::default(),
    }
}

/// 可执行假工具：执行即回显工具名（断言"放行后真的跑了"）。
struct ProbeTool(ToolSpec);

#[async_trait]
impl Tool for ProbeTool {
    fn spec(&self) -> &ToolSpec {
        &self.0
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(0, 1, 0);
        &V
    }

    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        Ok(json!({ "ran": self.0.name }))
    }
}

struct ProbeSource(Vec<ToolSpec>);

#[async_trait]
impl ToolSource for ProbeSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        self.0
            .iter()
            .find(|s| s.name == name)
            .map(|s| Arc::new(ProbeTool(s.clone())) as Arc<dyn Tool>)
            .ok_or_else(|| ToolError::NotAvailable(name.to_string()))
    }

    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        self.0.clone()
    }
}

/// 记录 consult 过的 (tool, side_effect) 并按预设决议。
struct RecordingResolver {
    decisions: ApprovalDecision,
    consulted: Mutex<Vec<String>>,
}

#[async_trait]
impl ApprovalResolver for RecordingResolver {
    async fn resolve(
        &self,
        req: &crate::kernel::ToolCallReq,
        _spec: &ToolSpec,
    ) -> ApprovalDecision {
        self.consulted.lock().unwrap().push(req.tool.clone());
        self.decisions.clone()
    }
}

/// 剧本 LLM：第一轮发出两个工具调用，第二轮纯文本收束。
struct ScriptedLlm {
    calls: Vec<ToolCallReq>,
}

#[async_trait]
impl LlmService for ScriptedLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: String::new(),
            tool_calls: self.calls.clone(),
            usage: None,
        })
    }
}

fn call(id: &str, tool: &str) -> ToolCallReq {
    ToolCallReq {
        id: CallId(id.into()),
        tool: tool.into(),
        args: json!({}),
        session_id: Some("s1".into()),
    }
}

fn specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "fs_write",
            SideEffect::Compensable {
                undo_tool: "fs_restore".into(),
            },
        ),
        spec("shell", SideEffect::Irreversible),
        spec("fs_read", SideEffect::Pure),
    ]
}

fn turn_tools() -> Vec<ToolCallReq> {
    vec![call("c1", "fs_write"), call("c2", "shell")]
}

/// 有决议者：Compensable 也被 consult（㉘·B `询问` 档 = 写类也要问）；
/// deny 时两个调用都以 denied ToolResult 闭合、各落一条 ApprovalDecided。
#[tokio::test]
async fn resolver_is_consulted_for_compensable_too() {
    let resolver = Arc::new(RecordingResolver {
        decisions: ApprovalDecision::deny("no"),
        consulted: Mutex::new(Vec::new()),
    });
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: turn_tools(),
    }));
    let kernel = Kernel::new(ctx, vec![]).with_approval(resolver.clone() as Arc<_>);
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    let mut consulted = resolver.consulted.lock().unwrap().clone();
    consulted.sort();
    assert_eq!(consulted, vec!["fs_write".to_string(), "shell".to_string()]);

    let denied = session
        .events()
        .iter()
        .filter(|e| matches!(e, SessionEvent::ToolResult { error: Some(err), .. } if err.contains("denied by approval")))
        .count();
    assert_eq!(denied, 2, "被拒调用逐个以 denied ToolResult 闭合");
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: false, .. }))
            .count(),
        2
    );
}

/// ㉘·C：决议者在场 = Pure 也被 consult（隐私门要能拦住只读工具读凭据）；
/// deny 时 Pure 调用同样以 denied ToolResult 闭合、落 ApprovalDecided。
#[tokio::test]
async fn resolver_is_consulted_for_pure_when_present() {
    let resolver = Arc::new(RecordingResolver {
        decisions: ApprovalDecision::deny("privacy gate"),
        consulted: Mutex::new(Vec::new()),
    });
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: vec![call("c1", "fs_read")],
    }));
    let kernel = Kernel::new(ctx, vec![]).with_approval(resolver.clone() as Arc<_>);
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    assert_eq!(
        resolver.consulted.lock().unwrap().clone(),
        vec!["fs_read".to_string()],
        "Pure 调用也过决策表"
    );
    assert!(session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { error: Some(err), .. }
                if err.contains("denied by approval: privacy gate"))
    }));
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::ApprovalDecided { allowed: false, .. }))
            .count(),
        1
    );
}

/// 无决议者（headless）：Compensable 照旧放行执行、Irreversible fail-safe
/// 拒绝——headless 语义零变化（纪律 9）。
#[tokio::test]
async fn no_resolver_compensable_runs_irreversible_denied() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: turn_tools(),
    }));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    let ran_compensable = session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { output, error: None, .. }
            if output.get("ran").and_then(Value::as_str) == Some("fs_write"))
    });
    assert!(ran_compensable, "无决议者时 Compensable 照旧放行执行");
    assert!(session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { error: Some(err), .. }
            if err.contains("no approval resolver configured"))
    }));
}

/// 无决议者（headless）：Pure 照旧静默放行——不 consult、不落
/// ApprovalDecided（㉘·C 的 headless 面保持）。
#[tokio::test]
async fn no_resolver_pure_runs_without_consult() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: vec![call("c1", "fs_read")],
    }));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    let ran = session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { output, error: None, .. }
            if output.get("ran").and_then(Value::as_str) == Some("fs_read"))
    });
    assert!(ran, "无决议者时 Pure 照旧放行执行");
    assert!(
        !session
            .events()
            .iter()
            .any(|e| matches!(e, SessionEvent::ApprovalDecided { .. })),
        "headless Pure 不产生审批事件"
    );
}

/// resolve 失败（目录围栏拒掉 / 工具不存在）以失败 ToolResult 闭合该调用、
/// 步骤继续——不留悬空（悬空的 tool_calls 会让下一轮请求被供应商拒收，
/// ㉘·B 计划模式走查实抓）。
#[tokio::test]
async fn resolve_failure_closes_result_without_dangling() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    // 剧本发出两个调用：fs_write（目录里有）+ ghost（不存在 → resolve 失败）
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: vec![call("c1", "fs_write"), call("c2", "ghost")],
    }));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    kernel
        .step_as(&mut session, "s1")
        .await
        .expect("turn ok（resolve 失败不再失败整轮）");

    // ghost 以失败 ToolResult 闭合；fs_write 无决议者放行正常执行
    let ghost_closed = session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { call_id, error: Some(err), .. }
            if call_id.0 == "c2" && err.contains("not available"))
    });
    assert!(ghost_closed, "resolve 失败必须以失败 ToolResult 闭合");
    let ran = session.events().iter().any(|e| {
        matches!(e, SessionEvent::ToolResult { call_id, output, error: None, .. }
            if call_id.0 == "c1" && output.get("ran").and_then(Value::as_str) == Some("fs_write"))
    });
    assert!(ran, "同轮其余调用照常执行");
    // 每个 tool_call 都有配对 ToolResult（供应商约束）
    for call_id in ["c1", "c2"] {
        assert!(
            session.events().iter().any(
                |e| matches!(e, SessionEvent::ToolResult { call_id: c, .. } if c.0 == call_id)
            ),
            "调用 {call_id} 必须有 ToolResult 闭合"
        );
    }
}

// ---- M1（KV v2 §4/§5）：相邻轮请求前缀一致（真实决策环集成）----

/// 逐次漂移的记忆源：每次召回 value = 调用序号——预取生效 = 只查一次。
struct DriftingMemory(std::sync::atomic::AtomicUsize);

impl MemorySource for DriftingMemory {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        vec![MemoryEntry {
            key: "drift".into(),
            value: n.to_string(),
            tags: String::new(),
            updated_at: 0,
        }]
    }
}

struct NoEwma;

impl GlobalEwma for NoEwma {
    fn ewma_scores(&self) -> std::collections::HashMap<String, f64> {
        std::collections::HashMap::new()
    }
}

/// 末位监听器：记录决策环出口 payload（LLM 实际收到的请求形状）。
struct Spy(Mutex<Vec<String>>);

#[async_trait]
impl WaterfallListener for Spy {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key == points::REQUEST {
            self.0.lock().unwrap().push(payload.to_string());
        }
        WaterfallDecision::Continue
    }
}

/// 剧本 LLM：第 1 次 complete 发一个 fs_read 调用，之后纯文本收束。
struct TurnOne {
    calls: Mutex<usize>,
}

#[async_trait]
impl LlmService for TurnOne {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut n = self.calls.lock().unwrap();
        let calls = if *n == 0 {
            vec![call("c1", "fs_read")]
        } else {
            vec![]
        };
        *n += 1;
        drop(n);
        Ok(LlmResponse {
            content: String::new(),
            tool_calls: calls,
            usage: None,
        })
    }
}

/// 同一会话连续两轮（共三次 LLM 请求）：tools 与 system 逐字节稳定。
/// 第 2 次请求起会话内出现过 fs_read（旧逐轮 Top-N 会把它换进暴露集）、
/// 记忆源逐次漂移（旧逐轮检索会改写 system）——钉死 + 预取让两者冻结，
/// 这是 M4 树模式 A/B 数据可信的前提。
#[tokio::test]
async fn adjacent_turns_keep_request_prefix_stable() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(TurnOne {
        calls: Mutex::new(0),
    }));
    // 决策环（注册次序 = 执行次序）：ToolExposure → Memory-Inject → Spy
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(ToolExposure::new(
            Arc::new(NoEwma),
            Arc::new(std::sync::RwLock::new(ToolExposureConfig {
                enabled: true,
                max_tools: 2,
                ..ToolExposureConfig::default()
            })),
            None,
        )) as Arc<_>,
    );
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            Arc::new(DriftingMemory(std::sync::atomic::AtomicUsize::new(0))),
            None,
            Arc::new(std::sync::RwLock::new(MemoryInjectConfig::default())),
        )),
    );
    let spy = Arc::new(Spy(Mutex::new(Vec::new())));
    ctx.events
        .on_waterfall(points::REQUEST, Arc::clone(&spy) as Arc<_>);

    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);
    let mut session = Session::new();
    // 轮 1：step 1 发 fs_read 调用（工具执行进会话使用记录），step 2 收束
    session.append(SessionEvent::UserMessage {
        content: "turn 1".into(),
    });
    kernel.step_as(&mut session, "s1").await.expect("step 1");
    kernel.step_as(&mut session, "s1").await.expect("step 2");
    // 轮 2：新用户消息
    session.append(SessionEvent::UserMessage {
        content: "turn 2".into(),
    });
    kernel.step_as(&mut session, "s1").await.expect("turn 2");

    let recorded = spy.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 3, "三次 LLM 请求各记一份出口 payload");
    let parsed: Vec<crate::RequestPayload> = recorded
        .iter()
        .map(|p| serde_json::from_str(p).expect("payload parses"))
        .collect();

    // tools 逐字节一致（含顺序）：fs_read 用过也不换入
    let tools_json: Vec<String> = parsed
        .iter()
        .map(|p| serde_json::to_string(&p.tools).unwrap())
        .collect();
    assert_eq!(tools_json[0], tools_json[1], "同轮相邻 step tools 冻结");
    assert_eq!(tools_json[1], tools_json[2], "跨轮 tools 冻结");
    let names: Vec<&str> = parsed[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["fs_write", "shell"], "首查选择钉死");

    // system（记忆块）逐字节一致：三次都是首查内容
    let systems: Vec<Option<String>> = parsed.iter().map(|p| p.system.clone()).collect();
    assert!(systems[0].is_some(), "记忆块已注入");
    assert_eq!(systems[0], systems[1], "相邻 step system 冻结");
    assert_eq!(systems[1], systems[2], "跨轮 system 冻结");
    assert!(
        systems[0].as_ref().unwrap().contains("drift = 0"),
        "块内容 = 首查值（预取后不再漂移）"
    );
}
