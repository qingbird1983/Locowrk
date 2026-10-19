//! 收敛判据回归（自 `tests.rs` 批 F-17 拆出，用例本体逐字未改）：报告用量的供应商下
//! 纯文本轮必须 1 步收束（M1 走查实抓的存量缺陷——判据曾读 events().last()），
//! 而真有工具调用的轮次不得被误判收敛。

use super::*;

use crate::events::AgentEvent;
use crate::runner::run_turn;
use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{Kernel, LlmRequest, LlmService, LlmUsage, ToolCallReq, ToolSource};
use plugin_api::CallId;
use serde_json::json;
use session_store::Session;
use std::sync::{Arc, Mutex as StdMutex};

/// 用量上报型剧本 LLM（v2.58 起真实供应商的常态：每步都带 LlmUsage）。
struct UsageLlm;

#[async_trait]
impl LlmService for UsageLlm {
    async fn complete(
        &self,
        _req: LlmRequest,
    ) -> Result<kernel_agent::LlmResponse, kernel_agent::LlmError> {
        Ok(kernel_agent::LlmResponse {
            content: "好的，收束。".into(),
            tool_calls: vec![],
            usage: Some(LlmUsage {
                input: 100,
                output: 10,
                cache_read: 90,
                cache_write: 0,
                reasoning: 0,
                total: 110,
            }),
        })
    }
}

/// M1 走查实抓（v2.58 存量缺陷回归）：报告用量的供应商下，收敛判据曾读
/// `events().last()`——LlmUsage 紧贴 assistant 落日志后 last() 恒为
/// LlmUsage，轮次永不收敛、每轮多跑一步（DeepSeek thinking 模式对以
/// assistant 结尾的请求 400 拒收，每轮尾部假 error）。回归 = 带用量的
/// 纯文本轮必须 1 步收束并发 TurnDone。
#[tokio::test]
async fn usage_reporting_provider_converges_on_final_text_step() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(SingleToolSource(Arc::new(EchoTool))));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(UsageLlm));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
        "你好".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("轮次必须正常收束");
    assert_eq!(
        steps, 1,
        "纯文本轮 1 步收束（不能被 LlmUsage 骗去多跑一步）"
    );
    assert!(
        sink.events()
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnDone { .. })),
        "正常收束必须发 TurnDone"
    );
}

/// 收束判据的直接补集：真有工具调用的轮次不能被误判收敛（判据改动不
/// 得放过未闭合的 tool_calls）。
#[tokio::test]
async fn usage_reporting_provider_still_loops_for_tool_calls() {
    struct ToolThenText(StdMutex<u32>);
    #[async_trait]
    impl LlmService for ToolThenText {
        async fn complete(
            &self,
            _req: LlmRequest,
        ) -> Result<kernel_agent::LlmResponse, kernel_agent::LlmError> {
            let mut n = self.0.lock().unwrap();
            let calls = if *n == 0 {
                vec![ToolCallReq {
                    id: CallId("c1".into()),
                    tool: "echo".into(),
                    args: json!({ "text": "hi" }),
                    session_id: None,
                }]
            } else {
                vec![]
            };
            *n += 1;
            drop(n);
            Ok(kernel_agent::LlmResponse {
                content: String::new(),
                tool_calls: calls,
                usage: Some(LlmUsage {
                    input: 100,
                    output: 10,
                    cache_read: 90,
                    cache_write: 0,
                    reasoning: 0,
                    total: 110,
                }),
            })
        }
    }

    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(SingleToolSource(Arc::new(EchoTool))));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ToolThenText(StdMutex::new(0))));
    let kernel = Kernel::new(ctx, vec![]);
    let mut session = Session::new();
    let sink = Collector::default();

    let steps = run_turn(
        &kernel,
        &mut session,
        "s1",
        "echo 一下".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("轮次必须正常收束");
    assert_eq!(steps, 2, "工具轮 = 调用步 + 收束步");
    assert!(
        sink.events()
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnDone { steps: Some(2), .. })),
        "工具轮收束发 TurnDone(2)"
    );
}
