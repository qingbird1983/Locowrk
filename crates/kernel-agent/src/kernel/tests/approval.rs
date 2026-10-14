//! 审批门 `consult` 范围（㉘·B/C）：非 Pure（含 Compensable）都过决议者；无决议者时
//! Compensable 照旧放行、Irreversible fail-safe 拒绝；决议者在场 = Pure 也过决策表
//! （隐私门要能拦住只读工具读凭据），headless 的 Pure 照旧静默放行。假件除共享区那几件
//! 外都是本簇私有：`ProbeTool` / `RecordingResolver` / `ScriptedLlm` / `turn_tools`。

use super::*;

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

/// 审批两时刻（队列 Q12·第二刀 v2.205）：等待发生在内核 `approve()` 那一次
/// `.await` 上，所以 `asked_at` / `decided_at` 由**内核**在调用前后各取一次
/// ——决议者实现（宿主审批门）不必为"带出请求时刻"改契约，headless 也不必。
/// 这一条钉住的是 active_ms 的数据源：日志里没有这两刻，前端就只能显示墙钟。
#[tokio::test]
async fn resolver_in_writes_both_approval_timestamps() {
    let resolver = Arc::new(RecordingResolver {
        decisions: ApprovalDecision::allow(),
        consulted: Mutex::new(Vec::new()),
    });
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: turn_tools(),
    }));
    let kernel = Kernel::new(ctx, vec![]).with_approval(resolver as Arc<_>);
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    let rows: Vec<(Option<i64>, Option<i64>)> = session
        .events()
        .iter()
        .filter_map(|e| match e {
            SessionEvent::ApprovalDecided {
                asked_at,
                decided_at,
                ..
            } => Some((*asked_at, *decided_at)),
            _ => None,
        })
        .collect();
    assert_eq!(rows.len(), 2, "两个调用各落一条决策：{rows:?}");
    for (asked, decided) in rows {
        let asked = asked.expect("有决议者 ⇒ asked_at 必须在场（那是等待时长的唯一源）");
        let decided = decided.expect("两刻同生同灭：有请求就有决策时刻");
        assert!(
            decided >= asked,
            "决策不得早于请求（时钟回拨要由投影层判成读不出，不是负等待）：{asked} → {decided}"
        );
    }
}

/// 无决议者的静默放行路径**不写时刻**（v2.205）：这一档的真相是"根本没有请求
/// 发生"，等待恒为 0；把它写成两刻相同（0 等待）与写成 None 的差别在于——
/// 投影层要能区分「知道没等待」与「不知道等了多久」，前者才允许 active_ms = 墙钟。
#[tokio::test]
async fn headless_passthrough_writes_no_timestamps() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm {
        calls: turn_tools(),
    }));
    let kernel = Kernel::new(ctx, vec![]); // 不带 with_approval = headless
    let mut session = Session::new();
    kernel.step_as(&mut session, "s1").await.expect("turn ok");

    let rows: Vec<(Option<i64>, Option<i64>)> = session
        .events()
        .iter()
        .filter_map(|e| match e {
            SessionEvent::ApprovalDecided {
                asked_at,
                decided_at,
                ..
            } => Some((*asked_at, *decided_at)),
            _ => None,
        })
        .collect();
    assert!(
        !rows.is_empty(),
        "无决议者时非 Pure 调用照旧落决策（纪律 9 的审计面不缩）"
    );
    assert!(
        rows.iter()
            .all(|(asked, decided)| asked.is_none() && decided.is_none()),
        "没有请求就没有时刻：{rows:?}"
    );
}
