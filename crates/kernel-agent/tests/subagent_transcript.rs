//! §3.16 子会话转录接收器契约（v2.52）：注入 `SubTranscriptSink` 后，
//! `run_child` 收束时把子**自身段**（任务 prompt 起，fork 来的父前缀不带）
//! 交给宿主——「子会话落盘」通路的内核侧契约。自带最小 rig（subagent.rs
//! 的 rig 已满 400 行纪律，不复用）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    ApprovalResolver, Kernel, LlmError, LlmResponse, LlmService, SubTranscriptSink, SubagentGate,
    SubagentTool, ToolCallReq, ToolSource,
};
use plugin_api::{CallId, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};
use session_store::{Session, SessionEvent, TurnOutcome};

mod common;

use common::{AllowAll, SeqLlm, user_session};

const SUB: &str = "subagent_run";

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

/// 收束回调记录：(child_id, 自身段事件副本)。
struct FakeSink(Mutex<Vec<(String, Vec<SessionEvent>)>>);

impl SubTranscriptSink for FakeSink {
    fn close(&self, child_id: &str, own_from: usize, child: &Session) {
        self.0
            .lock()
            .unwrap()
            .push((child_id.to_string(), child.events()[own_from..].to_vec()));
    }
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
        .expect("无成功闭合")
}

/// 最小装配：脚本 LLM + 仅 subagent_run 的目录 + 注入 sink。
fn rig(sink: Arc<FakeSink>, responses: Vec<Result<LlmResponse, LlmError>>) -> Arc<Kernel> {
    let ctx = Arc::new(Context::new());
    let gate = Arc::new(SubagentGate::default());
    let kernel = Arc::new(
        Kernel::new(ctx.clone(), vec![])
            .with_subagent(Arc::clone(&gate))
            .with_approval(Arc::new(AllowAll) as Arc<dyn ApprovalResolver>),
    );
    ctx.provide_dyn::<dyn LlmService>(Arc::new(SeqLlm::new(responses)) as Arc<dyn LlmService>);
    let sub: Arc<dyn Tool> = Arc::new(
        SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate)).with_transcript_sink(sink),
    );
    let map: HashMap<String, Arc<dyn Tool>> = [(SUB.to_string(), sub)].into_iter().collect();
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(DynSource(map)));
    kernel
}

/// 父派生一次的直接脚本（弹出顺序 = vec 尾先出）。
fn derive_then(
    child_text: Result<LlmResponse, LlmError>,
    parent_text: &str,
) -> Vec<Result<LlmResponse, LlmError>> {
    vec![
        SeqLlm::ok_text(parent_text),
        child_text,
        Ok(LlmResponse {
            content: "call #1".into(),
            tool_calls: vec![ToolCallReq {
                id: CallId("call-1".into()),
                tool: SUB.to_string(),
                args: json!({ "prompt": "落盘任务" }),
                session_id: None,
            }],
            usage: None,
        }),
    ]
}

#[tokio::test]
async fn transcript_sink_receives_own_segment() {
    let sink = Arc::new(FakeSink(Mutex::new(Vec::new())));
    let kernel = rig(
        sink.clone(),
        derive_then(SeqLlm::ok_text("子直接收敛"), "父收束"),
    );

    let mut s = user_session();
    kernel.step_as(&mut s, "p1").await.unwrap();
    let child_id = parent_result(&s)["child_session_id"]
        .as_str()
        .unwrap()
        .to_string();

    let got = sink.0.lock().unwrap();
    assert_eq!(got.len(), 1, "收束恰好回调一次");
    let (id, seg) = &got[0];
    assert_eq!(id, &child_id);
    // 自身段 = TurnStart + 任务 prompt + 收敛回复 + TurnEnd（v2.107 轮界
    // 时刻与主对话同款），fork 来的父前缀不带
    assert_eq!(seg.len(), 4, "自身段应恰为轮界一对夹任务轮：{seg:?}");
    assert!(
        matches!(seg.first(), Some(SessionEvent::TurnStart { ts, .. }) if *ts > 0),
        "段首应落 TurnStart（计时起点）：{seg:?}"
    );
    assert!(
        matches!(seg.get(1), Some(SessionEvent::UserMessage { content }) if content == "落盘任务"),
        "TurnStart 后应紧随任务 prompt：{seg:?}"
    );
    assert!(
        matches!(seg.get(2), Some(SessionEvent::AssistantMessage { tool_calls, .. }) if tool_calls.is_empty()),
        "段内应含收敛回复：{seg:?}"
    );
    match (seg.first(), seg.last()) {
        (
            Some(SessionEvent::TurnStart { ts: start, .. }),
            Some(SessionEvent::TurnEnd {
                ts: end,
                outcome: TurnOutcome::Completed,
                ..
            }),
        ) => assert!(end >= start, "TurnEnd 不得早于 TurnStart"),
        other => panic!("自身段首尾应是轮界事件对（Completed）：{other:?}"),
    }
}

/// v2.107 回归：报告用量的供应商（内核紧贴 assistant 落 `LlmUsage`）下，
/// 收敛判据不得只看 `events().last()`——旧代码子循环永不收敛、白跑到
/// 轮数上限（盘证：现网子代理收束 status 恒 budget_exhausted/failed）。
#[tokio::test]
async fn usage_reporting_provider_still_converges() {
    let sink = Arc::new(FakeSink(Mutex::new(Vec::new())));
    let kernel = rig(
        sink.clone(),
        derive_then(SeqLlm::ok_text_with_usage("子带用量收敛"), "父收束"),
    );

    let mut s = user_session();
    kernel.step_as(&mut s, "p1").await.unwrap();
    let out = parent_result(&s);
    assert_eq!(
        out["status"], "completed",
        "带用量也应在首个收敛步收束：{out}"
    );
    assert_eq!(out["turns"], 1, "一步收敛，不白跑：{out}");
}
