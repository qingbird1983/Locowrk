//! ⑮ 验收：子 Agent 调度——fork 父转录、白名单工具视图、单层硬剔除、
//! 预算/超时如实收束、Irreversible 冒泡带子 Agent 标识。
//! 注意 SeqLlm 是 pop()（LIFO）：脚本按**弹出顺序**倒排书写。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    ApprovalDecision, ApprovalResolver, Kernel, LlmError, LlmRequest, LlmResponse, LlmService,
    SubagentGate, SubagentTool, ToolCallReq, ToolSource,
};
use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};
use session_store::{Session, SessionEvent};

mod common;

use common::{AllowAll, MockTool, SeqLlm, user_session};

const SUB: &str = "subagent_run";

type Script = Vec<Result<LlmResponse, LlmError>>;

/// dyn 版固定目录（common::MockSource 只收具体 MockTool，这里要混装
/// SubagentTool）。
struct DynSource(HashMap<String, Arc<dyn Tool>>);

#[async_trait]
impl ToolSource for DynSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        self.0
            .get(name)
            .cloned()
            .ok_or_else(|| ToolError::NotAvailable(name.to_string()))
    }
    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        self.0.values().map(|t| t.spec().clone()).collect()
    }
}

/// 测试用审批记录器：恒允许，记录每次请求的会话身份。
struct RecordingResolver(Mutex<Vec<Option<String>>>);

#[async_trait]
impl ApprovalResolver for RecordingResolver {
    async fn resolve(&self, req: &ToolCallReq, _spec: &ToolSpec) -> ApprovalDecision {
        self.0.lock().unwrap().push(req.session_id.clone());
        ApprovalDecision::allow()
    }
}

/// 睡眠 LLM（超时测试）：每次请求睡固定时长再弹脚本。
struct SleepyLlm {
    delay: Duration,
    script: Mutex<Script>,
}

#[async_trait]
impl LlmService for SleepyLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        tokio::time::sleep(self.delay).await;
        self.script
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| Err(LlmError::Backend("script exhausted".into())))
    }
}

/// 组装带 subagent_run 的内核（工具面 + 可选审批决议者）。
struct Rig {
    kernel: Arc<Kernel>,
    llm: Arc<SeqLlm>,
}

fn rig(
    tools: Vec<Arc<MockTool>>,
    script: Script,
    resolver: Option<Arc<dyn ApprovalResolver>>,
) -> Rig {
    let ctx = Arc::new(Context::new());
    let gate = Arc::new(SubagentGate::default());
    // subagent_run 是 Irreversible：缺省 AllowAll 让用例聚焦调度本身
    // （审批 fail-safe 另有专项）；传入决议者则整体替换（冒泡测试用）
    let k = Kernel::new(ctx.clone(), vec![])
        .with_subagent(Arc::clone(&gate))
        .with_approval(resolver.unwrap_or_else(|| Arc::new(AllowAll) as Arc<dyn ApprovalResolver>));
    let kernel = Arc::new(k);
    let llm = Arc::new(SeqLlm::new(script));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    let sub: Arc<dyn Tool> = Arc::new(SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate)));
    let mut all: Vec<Arc<dyn Tool>> = tools.into_iter().map(|t| t as Arc<dyn Tool>).collect();
    all.push(sub);
    let map = all
        .into_iter()
        .map(|t| (t.spec().name.clone(), t))
        .collect();
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(DynSource(map)));
    Rig { kernel, llm }
}

/// 父会话的 subagent_run 结果（最后一个成功 ToolResult 的 output）。
fn parent_result(s: &Session) -> Value {
    s.events()
        .iter()
        .rev()
        .find_map(|e| match e {
            SessionEvent::ToolResult {
                output,
                error: None,
                ..
            } => Some(output.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "无成功闭合；尾部事件：{:?}",
                &s.events()[s.len().saturating_sub(4)..]
            )
        })
}

#[tokio::test]
async fn subagent_forks_transcript_and_returns_summary() {
    let echo = Arc::new(MockTool::new("echo", SideEffect::Pure, json!("e-out")));
    // 弹出顺序：父派生 → 子调 echo → 子收敛
    let rig = rig(
        vec![Arc::clone(&echo)],
        vec![
            SeqLlm::ok_text("done summary"),
            SeqLlm::call_tool(2, "echo"),
            sub_call(1, json!({ "prompt": "子任务" })),
        ],
        None,
    );
    let mut s = user_session();
    rig.kernel.step_as(&mut s, "p1").await.unwrap();

    let out = parent_result(&s);
    assert_eq!(out["status"], json!("completed"));
    assert_eq!(out["summary"], json!("done summary"));
    assert_eq!(out["turns"], json!(2));
    assert_eq!(out["tools_used"], json!(["echo"]));
    let child_id = out["child_session_id"].as_str().unwrap();
    assert!(child_id.starts_with("p1-sub-"), "got {child_id}");

    // 子目录 = 继承父全目录但硬剔除自身；消息 = 父转录前缀 + 任务 prompt
    let reqs = rig.llm.requests.lock().unwrap();
    let child_catalog: Vec<String> = reqs[1].tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(
        child_catalog,
        vec!["echo".to_owned()],
        "subagent_run 不进子目录"
    );
    let last = format!("{:?}", reqs[1].messages.last().unwrap());
    assert!(last.contains("子任务"), "子最后一条是任务 prompt：{last}");
    assert!(reqs[1].messages.len() >= 2, "fork 了父转录前缀");
    assert_eq!(echo.calls(), 1);
}

#[tokio::test]
async fn explicit_whitelist_blocks_unlisted_resolve() {
    let email = Arc::new(MockTool::new("send_email", SideEffect::Pure, json!("m")));
    // 弹出顺序：父派生（tools 只许 echo）→ 子越权调 send_email（围栏拒解析）
    // → 子以失败 ToolResult 闭合继续收束（㉘·B：解析失败不再失败整轮）
    let rig = rig(
        vec![email.clone()],
        vec![
            SeqLlm::ok_text("never"),
            SeqLlm::call_tool(2, "send_email"),
            sub_call(1, json!({ "prompt": "子任务", "tools": ["echo"] })),
        ],
        None,
    );
    let mut s = user_session();
    rig.kernel.step_as(&mut s, "p1").await.unwrap();

    let out = parent_result(&s);
    assert_eq!(out["status"], json!("completed"), "子轮照常收束：{out}");
    // 围栏可证：越权工具从未执行（resolve 在 ToolCall 落账前被拒）
    assert_eq!(email.calls(), 0, "白名单外的工具不得执行");
    assert_eq!(out["tools_used"], json!([]), "越权调用不进 tools_used");
}

#[tokio::test]
async fn single_level_hard_block_even_if_allowlisted() {
    let echo = Arc::new(MockTool::new("echo", SideEffect::Pure, json!("e")));
    // 白名单显式包含 subagent_run：目录层照样剔除（子不可再生子）
    let rig = rig(
        vec![echo],
        vec![
            SeqLlm::ok_text("never"),
            SeqLlm::call_tool(2, SUB),
            sub_call(1, json!({ "prompt": "再生子", "tools": [SUB] })),
        ],
        None,
    );
    let mut s = user_session();
    rig.kernel.step_as(&mut s, "p1").await.unwrap();

    // ㉘·B：解析失败以失败 ToolResult 闭合、子轮继续收束——硬底线不变：
    // 子的 subagent_run 调用从未解析成功（不可再生子），只是收束语义更诚实
    let out = parent_result(&s);
    assert_eq!(out["status"], json!("completed"), "子轮照常收束：{out}");
    assert_eq!(
        out["tools_used"],
        json!([]),
        "单层硬底线：subagent_run 未执行"
    );
}

#[tokio::test]
async fn budget_exhausted_reports_honestly() {
    let echo = Arc::new(MockTool::new("echo", SideEffect::Pure, json!("e")));
    // 弹出顺序：父派生（max_turns=1）→ 子第 1 步仍在调工具（不收敛）→ 截断
    let rig = rig(
        vec![echo],
        vec![
            SeqLlm::call_tool(2, "echo"),
            sub_call(1, json!({ "prompt": "长任务", "max_turns": 1 })),
        ],
        None,
    );
    let mut s = user_session();
    rig.kernel.step_as(&mut s, "p1").await.unwrap();

    let out = parent_result(&s);
    assert_eq!(out["status"], json!("budget_exhausted"));
    assert_eq!(out["turns"], json!(1));
    // 子第 1 步正文非空（"call #2"）：摘要如实取转录尾部，而非编造动作
    assert_eq!(out["summary"], json!("call #2"));
}

#[tokio::test]
async fn timeout_reports_honestly() {
    let ctx = Arc::new(Context::new());
    let gate = Arc::new(SubagentGate::default());
    let kernel = Arc::new(
        Kernel::new(ctx.clone(), vec![])
            .with_subagent(Arc::clone(&gate))
            .with_approval(Arc::new(AllowAll)),
    );
    // 每次请求 1500ms；子循环带 1s 超时 → 第一个子请求就超时
    // （父请求同样慢但不受预算约束，总耗时 ~3s 可接受）
    let slow = Arc::new(SleepyLlm {
        delay: Duration::from_millis(1500),
        script: Mutex::new(vec![
            SeqLlm::call_tool(2, "echo"),
            sub_call(1, json!({ "prompt": "慢任务", "timeout_secs": 1 })),
        ]),
    });
    ctx.provide_dyn::<dyn LlmService>(slow as Arc<dyn LlmService>);
    let echo = Arc::new(MockTool::new("echo", SideEffect::Pure, json!("e")));
    let sub: Arc<dyn Tool> = Arc::new(SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate)));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(DynSource(HashMap::from([
        (echo.spec().name.clone(), echo as Arc<dyn Tool>),
        (sub.spec().name.clone(), sub),
    ]))));

    let mut s = user_session();
    kernel.step_as(&mut s, "p1").await.unwrap();

    let out = parent_result(&s);
    assert_eq!(out["status"], json!("timeout"), "子请求 1500ms > 1s 预算");
}

#[tokio::test]
async fn child_irreversible_bubbles_with_subagent_tag() {
    let bomb = Arc::new(MockTool::new(
        "danger",
        SideEffect::Irreversible,
        json!("boom"),
    ));
    // 弹出顺序：父派生（tools 只许 danger）→ 子调 danger（过审批门）→ 收敛
    let recorder = Arc::new(RecordingResolver(Mutex::new(vec![])));
    let rig = rig(
        vec![Arc::clone(&bomb)],
        vec![
            SeqLlm::ok_text("all done"),
            SeqLlm::call_tool(2, "danger"),
            sub_call(1, json!({ "prompt": "拆弹", "tools": ["danger"] })),
        ],
        Some(Arc::clone(&recorder) as Arc<dyn ApprovalResolver>),
    );
    let mut s = user_session();
    rig.kernel.step_as(&mut s, "p1").await.unwrap();

    let out = parent_result(&s);
    assert_eq!(out["status"], json!("completed"));
    assert_eq!(bomb.calls(), 1, "审批放行后子内工具执行");
    let seen = recorder.0.lock().unwrap();
    assert!(
        seen.iter()
            .any(|id| id.as_ref().is_some_and(|s| s.starts_with("p1-sub-"))),
        "审批请求带子 Agent 标识：{seen:?}"
    );
}

#[tokio::test]
async fn tool_requires_session_identity_and_prompt() {
    let ctx = Arc::new(Context::new());
    let gate = Arc::new(SubagentGate::default());
    let kernel = Arc::new(Kernel::new(ctx.clone(), vec![]));
    let tool = SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate));

    // 无会话身份：直接报错（子 Agent 只能在会话内派生）
    let err = tool
        .call(&CallCtx::new(), json!({ "prompt": "x" }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("session_id"));

    // 有身份但 prompt 缺失：参数校验报错
    let mut cctx = CallCtx::new();
    cctx.session_id = Some("p1".into());
    let err = tool.call(&cctx, json!({})).await.unwrap_err();
    assert!(err.to_string().contains("prompt"));
}

#[tokio::test]
async fn no_session_snapshot_fails_honestly() {
    let ctx = Arc::new(Context::new());
    let gate = Arc::new(SubagentGate::default());
    let kernel = Arc::new(Kernel::new(ctx.clone(), vec![]).with_subagent(Arc::clone(&gate)));
    let tool = SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate));

    // ghost 身份从未在本内核走过 step：无快照可 fork，如实报错（非 panic）
    let mut cctx = CallCtx::new();
    cctx.session_id = Some("ghost".into());
    let out = tool.call(&cctx, json!({ "prompt": "x" })).await.unwrap();
    assert_eq!(out["status"], json!("failed"));
    assert!(out["error"].as_str().unwrap().contains("快照"));
}

#[tokio::test]
async fn tools_used_ignores_forked_parent_history() {
    let echo = Arc::new(MockTool::new("echo", SideEffect::Pure, json!("e")));
    // 弹出顺序：父派生 → 子调 echo → 子收敛
    let rig = rig(
        vec![Arc::clone(&echo)],
        vec![
            SeqLlm::ok_text("done"),
            SeqLlm::call_tool(2, "echo"),
            sub_call(1, json!({ "prompt": "子任务", "tools": ["echo"] })),
        ],
        None,
    );
    // 父史：早前轮次调过 mcp_status（fork 前缀会带上这段转录）——
    // 汇报的 tools_used 只计子内自身的 echo，不把父的调用史算进来
    // （smoke 实测曾把父的 mcp_status 误报成子的使用）。
    let mut s = user_session();
    s.append(SessionEvent::ToolCall {
        call_id: CallId("old-1".into()),
        tool: "mcp_status".into(),
        args: json!({}),
        plugin_id: None,
        plugin_version: semver::Version::new(0, 0, 0),
        side_effect: SideEffect::Pure,
    });
    s.append(SessionEvent::ToolResult {
        call_id: CallId("old-1".into()),
        output: json!({}),
        error: None,
    });
    rig.kernel.step_as(&mut s, "p1").await.unwrap();

    let out = parent_result(&s);
    assert_eq!(out["tools_used"], json!(["echo"]), "父前缀历史调用不计入");
    assert_eq!(out["summary"], json!("done"), "摘要取子自身段正文");
}

/// 造一条带参的 subagent_run 调用脚本响应。
fn sub_call(n: usize, args: Value) -> Result<LlmResponse, LlmError> {
    Ok(LlmResponse {
        content: format!("call #{n}"),
        tool_calls: vec![ToolCallReq {
            id: CallId(format!("call-{n}")),
            tool: SUB.to_string(),
            args,
            session_id: None,
        }],
        usage: None,
    })
}
