//! §3.16 子会话转录接收器契约（v2.52）：注入 `SubTranscriptSink` 后，
//! `run_child` 收束时把子**自身段**（任务 prompt 起，fork 来的父前缀不带）
//! 交给宿主——「子会话落盘」通路的内核侧契约。自带最小 rig（subagent.rs
//! 的 rig 已满 400 行纪律，不复用）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    ApprovalResolver, Kernel, LlmResponse, LlmService, SubTranscriptSink, SubagentGate,
    SubagentTool, ToolCallReq, ToolSource,
};
use plugin_api::{CallId, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};
use session_store::{Session, SessionEvent};

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

/// 收束回调记录：(child_id, 自身段条数, 首条内容前缀)。
struct FakeSink(Mutex<Vec<(String, usize, String)>>);

impl SubTranscriptSink for FakeSink {
    fn close(&self, child_id: &str, own_from: usize, child: &Session) {
        let first = match child.events().get(own_from) {
            Some(SessionEvent::UserMessage { content }) => format!("user:{content}"),
            Some(other) => format!("other:{other:?}"),
            None => "empty".into(),
        };
        self.0
            .lock()
            .unwrap()
            .push((child_id.to_string(), child.len() - own_from, first));
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

#[tokio::test]
async fn transcript_sink_receives_own_segment() {
    let sink = Arc::new(FakeSink(Mutex::new(Vec::new())));

    let ctx = Arc::new(Context::new());
    let gate = Arc::new(SubagentGate::default());
    let kernel = Arc::new(
        Kernel::new(ctx.clone(), vec![])
            .with_subagent(Arc::clone(&gate))
            .with_approval(Arc::new(AllowAll) as Arc<dyn ApprovalResolver>),
    );
    // 弹出顺序：父派生 → 子直接收敛
    let derive = Ok(LlmResponse {
        content: "call #1".into(),
        tool_calls: vec![ToolCallReq {
            id: CallId("call-1".into()),
            tool: SUB.to_string(),
            args: json!({ "prompt": "落盘任务" }),
            session_id: None,
        }],
        usage: None,
    });
    let llm = Arc::new(SeqLlm::new(vec![
        SeqLlm::ok_text("父收束"),
        SeqLlm::ok_text("子直接收敛"),
        derive,
    ]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm) as Arc<dyn LlmService>);
    let sub: Arc<dyn Tool> = Arc::new(
        SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate))
            .with_transcript_sink(sink.clone()),
    );
    let map: HashMap<String, Arc<dyn Tool>> = [(SUB.to_string(), sub)].into_iter().collect();
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(DynSource(map)));

    let mut s = user_session();
    kernel.step_as(&mut s, "p1").await.unwrap();
    let child_id = parent_result(&s)["child_session_id"]
        .as_str()
        .unwrap()
        .to_string();

    let got = sink.0.lock().unwrap();
    assert_eq!(got.len(), 1, "收束恰好回调一次");
    let (id, own_len, first) = &got[0];
    assert_eq!(id, &child_id);
    assert_eq!(*own_len, 2, "自身段 = 任务 prompt + 收敛回复，父前缀不带");
    assert_eq!(first, "user:落盘任务");
}
