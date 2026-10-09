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
use crate::kernel::{TreeContextSource, TreePlan, collapse_expanded, tail_user_start};
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
            scope: String::new(),
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
        parts: Vec::new(),
    });
    kernel.step_as(&mut session, "s1").await.expect("step 1");
    kernel.step_as(&mut session, "s1").await.expect("step 2");
    // 轮 2：新用户消息
    session.append(SessionEvent::UserMessage {
        content: "turn 2".into(),
        parts: Vec::new(),
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

// ---- M4 记忆树切流（树 v2 §11.2 阶段 2）：树会话组装 / legacy 零变化 ----

/// 剧本树组装源：返回固定计划（查无此会话 = legacy）。
struct FakeTrees(Mutex<std::collections::HashMap<String, TreePlan>>);

impl FakeTrees {
    fn with(id: &str, plan: TreePlan) -> Self {
        Self(Mutex::new([(id.to_string(), plan)].into_iter().collect()))
    }
}

#[async_trait]
impl TreeContextSource for FakeTrees {
    async fn plan(&self, session_id: &str) -> Option<TreePlan> {
        self.0.lock().unwrap().get(session_id).cloned()
    }
}

/// 纯文本 LLM。
struct TextLlm;

#[async_trait]
impl LlmService for TextLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: "ok".into(),
            tool_calls: vec![],
            usage: None,
        })
    }
}

fn tail_plan(tail_from: usize, directory: &str, collapsed: &[&str]) -> TreePlan {
    TreePlan {
        directory: directory.into(),
        tail_from_event: tail_from,
        collapsed_chapters: collapsed.iter().map(|s| s.to_string()).collect(),
    }
}

/// 组装 kernel：TextLlm + Spy 出口记录。
fn echo_kernel(ctx: &Arc<Context>) -> (Kernel, Arc<Spy>) {
    let spy = Arc::new(Spy(Mutex::new(Vec::new())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(TextLlm));
    ctx.events
        .on_waterfall(points::REQUEST, Arc::clone(&spy) as Arc<_>);
    (Kernel::new(Arc::clone(ctx), vec![]), spy)
}

#[tokio::test]
async fn tree_session_slices_to_tail_and_appends_directory() {
    let ctx = Arc::new(Context::new());
    // Memory-Inject 在前 → TreeDirectoryInject 在后（目录在记忆块之后）
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            Arc::new(DriftingMemory(std::sync::atomic::AtomicUsize::new(0))),
            Arc::new(std::sync::RwLock::new(MemoryInjectConfig::default())),
        )) as Arc<_>,
    );
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::TreeDirectoryInject) as Arc<_>,
    );
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(
            2,
            "[memory tree] Earlier turns are organized below (oldest first); call memory_expand(chapter_id, level=1|2) to read one.\n- b2: 缓存讨论",
            &[],
        ),
    )));
    let (kernel, spy) = echo_kernel(&ctx);

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "early".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "early reply".into(),
        tool_calls: vec![],
    });
    s.append(SessionEvent::UserMessage {
        content: "tail q".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "tail a".into(),
        tool_calls: vec![],
    });
    kernel.step_as(&mut s, "s1").await.expect("step");

    let recorded = spy.0.lock().unwrap().clone();
    let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    // 消息 = 尾巴两条；system = 记忆块 + 目录（次序 = 树 v2 §7.1）
    assert_eq!(payload.messages.len(), 2);
    assert!(matches!(
        &payload.messages[0],
        session_store::Message::User { content, .. } if content == "tail q"
    ));
    let system = payload.system.expect("system 在场");
    let mem_at = system.find("<persistent_memory>").expect("记忆块在前");
    let tree_at = system.find("[memory tree]").expect("目录在后");
    assert!(mem_at < tree_at, "目录必须排在记忆块之后（§7.1 [2]→[3]）");
    assert!(
        system.ends_with("- b2: 缓存讨论"),
        "目录挂 system 尾部（清单行收尾，无闭合标签——闭合标签会随追加移动破坏前缀）"
    );
    assert!(payload.tree_directory.is_some(), "树会话标记随 payload 走");
}

#[tokio::test]
async fn tree_session_midturn_boundary_advances_to_next_user() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        // 切点落在助手消息上（记忆管理器切分不硬校验的兜底场景）
        tail_plan(1, "", &[]),
    )));
    let (kernel, spy) = echo_kernel(&ctx);

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "u1".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "a1".into(),
        tool_calls: vec![],
    });
    s.append(SessionEvent::UserMessage {
        content: "u2".into(),
        parts: Vec::new(),
    });
    kernel.step_as(&mut s, "s1").await.expect("step");

    let recorded = spy.0.lock().unwrap().clone();
    let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    assert_eq!(payload.messages.len(), 1, "前进到第一条 User：只剩 u2");
    assert!(matches!(
        &payload.messages[0],
        session_store::Message::User { content, .. } if content == "u2"
    ));
    assert_eq!(payload.system, None, "空目录不追加 system");
}

#[tokio::test]
async fn tree_session_collapsed_expand_results_become_stubs() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(0, "", &["b2"]),
    )));
    let (kernel, spy) = echo_kernel(&ctx);

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "q".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: String::new(),
        tool_calls: vec![CallId("c1".into()), CallId("c2".into())],
    });
    for (cid, chapter) in [("c1", "b1"), ("c2", "b2")] {
        s.append(SessionEvent::ToolCall {
            call_id: CallId(cid.into()),
            tool: "memory_expand".into(),
            args: json!({ "chapter_id": chapter, "level": 1 }),
            plugin_id: None,
            plugin_version: Version::new(0, 1, 0),
            side_effect: SideEffect::Pure,
        });
        s.append(SessionEvent::ToolResult {
            call_id: CallId(cid.into()),
            output: json!({ "summary": "很长很长的原文".to_string() }),
            error: None,
        });
    }
    kernel.step_as(&mut s, "s1").await.expect("step");

    let recorded = spy.0.lock().unwrap().clone();
    let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    let mut stubbed = 0;
    for m in &payload.messages {
        if let session_store::Message::ToolResult {
            tool, args, output, ..
        } = m
        {
            assert_eq!(tool, "memory_expand");
            let id = args["chapter_id"].as_str().unwrap();
            if id == "b2" {
                assert!(
                    output.as_str().unwrap().contains("collapsed"),
                    "收起章节的结果换成桩文本"
                );
                stubbed += 1;
            } else {
                assert!(output.is_object(), "未收起章节原样保留");
            }
        }
    }
    assert_eq!(stubbed, 1, "只桩 b2");
    assert_eq!(payload.messages.len(), 4, "tool_calls ↔ 结果配对不拆");
}

#[tokio::test]
async fn tree_session_skips_compaction_over_threshold() {
    let low = || crate::CompactConfig {
        threshold_tokens: 1,
        ..Default::default()
    };
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(0, "", &[]),
    )));
    // 阈值压到极低：legacy 会话必压；树会话必须跳过
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::CtxCompact::new(
            Arc::clone(&ctx),
            Arc::new(std::sync::RwLock::new(low())),
        )) as Arc<_>,
    );
    let (kernel, spy) = echo_kernel(&ctx);

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "u1".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "a1".into(),
        tool_calls: vec![],
    });
    s.append(SessionEvent::UserMessage {
        content: "u2".into(),
        parts: Vec::new(),
    });
    kernel.step_as(&mut s, "s1").await.expect("step");

    let recorded = spy.0.lock().unwrap().clone();
    let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    assert!(payload.compacted.is_none(), "树会话不压缩");
    assert_eq!(payload.messages.len(), 3, "消息原样");

    // 同一批事件在 legacy 会话（无计划）上照常压缩
    let ctx2 = Arc::new(Context::new());
    ctx2.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::CtxCompact::new(
            Arc::clone(&ctx2),
            Arc::new(std::sync::RwLock::new(low())),
        )) as Arc<_>,
    );
    let (kernel2, spy2) = echo_kernel(&ctx2);
    let mut s2 = Session::new();
    s2.append(SessionEvent::UserMessage {
        content: "u1".into(),
        parts: Vec::new(),
    });
    s2.append(SessionEvent::AssistantMessage {
        content: "a1".into(),
        tool_calls: vec![],
    });
    s2.append(SessionEvent::UserMessage {
        content: "u2".into(),
        parts: Vec::new(),
    });
    kernel2.step_as(&mut s2, "s1").await.expect("step");
    let recorded2 = spy2.0.lock().unwrap().clone();
    let payload2: crate::RequestPayload = serde_json::from_str(&recorded2[0]).unwrap();
    assert!(payload2.compacted.is_some(), "legacy 会话压缩照常触发");
}

#[tokio::test]
async fn legacy_sessions_keep_full_assembly() {
    // ① 组装源未注册（headless）② 计划返回 None（legacy 会话）：全量投影
    for with_source in [false, true] {
        let ctx = Arc::new(Context::new());
        if with_source {
            ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees(Mutex::new(
                std::collections::HashMap::new(),
            ))));
        }
        let (kernel, spy) = echo_kernel(&ctx);
        let mut s = Session::new();
        s.append(SessionEvent::UserMessage {
            content: "u1".into(),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: "a1".into(),
            tool_calls: vec![],
        });
        s.append(SessionEvent::UserMessage {
            content: "u2".into(),
            parts: Vec::new(),
        });
        kernel.step_as(&mut s, "s1").await.expect("step");
        let recorded = spy.0.lock().unwrap().clone();
        let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
        assert_eq!(payload.messages.len(), 3);
        assert_eq!(payload.system, None);
        assert_eq!(payload.tree_directory, None);
    }
}

#[tokio::test]
async fn directory_growth_keeps_system_prefix() {
    // 只增不改（纪律 3）：章节 b3 追加后，旧 system 必须是新 system 的字节前缀
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(
            1,
            "[memory tree] Earlier turns are organized below.\n- b2: 旧章节",
            &[],
        ),
    )));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::TreeDirectoryInject) as Arc<_>,
    );
    let (kernel, spy) = echo_kernel(&ctx);
    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "u1".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "a1".into(),
        tool_calls: vec![],
    });
    kernel.step_as(&mut s, "s1").await.expect("step 1");

    // b3 出现：目录追加一行
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(
            1,
            "[memory tree] Earlier turns are organized below.\n- b2: 旧章节\n- b3: 新章节",
            &[],
        ),
    )));
    s.append(SessionEvent::UserMessage {
        content: "u2".into(),
        parts: Vec::new(),
    });
    kernel.step_as(&mut s, "s1").await.expect("step 2");

    let recorded = spy.0.lock().unwrap().clone();
    let v1: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    let v2: crate::RequestPayload = serde_json::from_str(&recorded[1]).unwrap();
    let s1 = v1.system.unwrap();
    let s2 = v2.system.unwrap();
    assert!(
        s2.starts_with(&s1),
        "目录只增不改：旧前缀逐字节保留 (s1={s1:?} s2={s2:?})"
    );
}

#[test]
fn tail_user_start_and_collapse_helpers() {
    let events = vec![
        SessionEvent::UserMessage {
            content: "u".into(),
            parts: Vec::new(),
        },
        SessionEvent::AssistantMessage {
            content: "a".into(),
            tool_calls: vec![],
        },
        SessionEvent::UserMessage {
            content: "u2".into(),
            parts: Vec::new(),
        },
    ];
    assert_eq!(tail_user_start(&events, 0), Some(0));
    assert_eq!(tail_user_start(&events, 1), Some(2), "半轮起点前进到 User");
    assert_eq!(tail_user_start(&events, 3), None);
    assert_eq!(tail_user_start(&events, 99), None, "越界安全");

    let msgs = vec![session_store::Message::ToolResult {
        call_id: CallId("c1".into()),
        tool: "memory_expand".into(),
        args: json!({ "chapter_id": "b9" }),
        output: json!({ "summary": "x" }),
        error: None,
    }];
    let out = collapse_expanded(msgs.clone(), &["b2".into()]);
    assert!(out[0].output_is_object(), "不在收起集 = 原样保留");
    let out = collapse_expanded(msgs, &["b9".into()]);
    assert!(out[0].output_contains_stub(), "收起集命中 = 桩文本");
}

/// 测试小工具：ToolResult output 形态断言。
trait OutputShape {
    fn output_is_object(&self) -> bool;
    fn output_contains_stub(&self) -> bool;
}
impl OutputShape for session_store::Message {
    fn output_is_object(&self) -> bool {
        matches!(self, session_store::Message::ToolResult { output, .. } if output.is_object())
    }
    fn output_contains_stub(&self) -> bool {
        matches!(self, session_store::Message::ToolResult { output, .. }
            if output.as_str().is_some_and(|s| s.contains("collapsed")))
    }
}
