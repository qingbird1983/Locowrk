//! specs/04 · C1 的验收（台账 T13 的第一刀）：**取消信号能不能落到正在跑的那次调用手里**。
//!
//! 设计稿明写了这条的判据 = 一条**行为**用例，不要结构断言（"字段终于不是 None 了"那种
//! 不证明任何事）。所以这里的观测点是替身工具自己返回的形状：
//! 早停在第几轮 vs 跑满 200 轮，两种结果在 JSON 里长得不一样；
//! 外加登记表的两条边界——置位条数（在途 = 1）与收束后归零（RAII 真注销了）。
//!
//! 三条各抓一层，互不重叠：
//!  ① 在途调用真的收到取消（跨任务：另一个任务等工具跑起来后按会话停止）；
//!  ② 调用收束后表里不再有位（`cancel_session` 再点一次 = 0）——漏注销的那条会让
//!     "置位了几条"这个读数开始说谎，而它谎报时没有任何报错；
//!  ③ 只动本会话 + 序号在位（同一 call_id 重叠登记时，先收束那条不许抹掉后一条）。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    ActiveCalls, Kernel, LlmError, LlmRequest, LlmResponse, LlmService, StepOutcome, ToolCallReq,
    ToolSource,
};
use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};
use session_store::{Message, Session, SessionEvent};

static TOOL_VERSION: semver::Version = semver::Version::new(0, 1, 0);

/// 会检查取消标志的替身工具：每轮睡 5ms，最多 `rounds` 轮。
///
/// `started` 是测试与它之间唯一的握手点（"我进到循环里了"）——没有它，另一侧的
/// 停止可能发生在调用**开始之前**，那条用例就会"绿得什么都没测到"。
struct PollTool {
    spec: ToolSpec,
    started: Arc<AtomicUsize>,
    rounds: u32,
}

#[async_trait]
impl Tool for PollTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }
    async fn call(&self, ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        self.started.fetch_add(1, Ordering::Relaxed);
        for round in 0..self.rounds {
            if ctx.is_cancelled() {
                // 早停的形状：与"跑完"在 JSON 上**可分辨**，不是靠日志肉眼比对。
                return Ok(json!({ "early_stop": true, "round": round }));
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok(json!({ "early_stop": false, "round": self.rounds }))
    }
}

fn poll_spec() -> ToolSpec {
    ToolSpec {
        name: "poll".into(),
        description: "polls the cancel flag".into(),
        schema: json!({}),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    }
}

struct OneSource {
    tool: Arc<PollTool>,
}

#[async_trait]
impl ToolSource for OneSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        if name == "poll" {
            return Ok(Arc::clone(&self.tool) as Arc<dyn Tool>);
        }
        Err(ToolError::NotAvailable(name.into()))
    }
    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        vec![poll_spec()]
    }
}

/// 脚本 LLM：第一步要求调 `poll`，看到工具结果就给最终答案。
struct ScriptedLlm;

#[async_trait]
impl LlmService for ScriptedLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let saw_result = req
            .messages
            .iter()
            .any(|m| matches!(m, Message::ToolResult { .. }));
        Ok(if saw_result {
            LlmResponse {
                content: "final".into(),
                tool_calls: vec![],
                usage: None,
            }
        } else {
            LlmResponse {
                content: "need tool".into(),
                tool_calls: vec![ToolCallReq {
                    id: CallId("call-1".into()),
                    tool: "poll".into(),
                    args: json!({}),
                    session_id: None,
                }],
                usage: None,
            }
        })
    }
}

fn ctx_with(session: Option<&str>, call: Option<&str>) -> CallCtx {
    let mut c = CallCtx::new();
    c.session_id = session.map(str::to_owned);
    c.call_id = call.map(|id| CallId(id.to_string()));
    c
}

/// 从会话日志里取那次调用的 ToolResult 载荷。
fn tool_result_output(session: &Session, call_id: &str) -> Option<Value> {
    session.events().iter().find_map(|e| match e {
        SessionEvent::ToolResult {
            call_id: id,
            output,
            error,
        } if id.0 == call_id => {
            let _ = error;
            Some(output.clone())
        }
        _ => None,
    })
}

#[tokio::test]
async fn stop_reaches_the_call_in_flight_and_the_table_empties_after() {
    let started = Arc::new(AtomicUsize::new(0));
    let tool = Arc::new(PollTool {
        spec: poll_spec(),
        started: Arc::clone(&started),
        // 200 轮 × 5ms = 最多 1 秒；收不到取消时用例靠这个上界自己收束，不永挂
        rounds: 200,
    });
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn LlmService>(Arc::new(ScriptedLlm));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(OneSource {
        tool: Arc::clone(&tool),
    }));
    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);
    let active = kernel.active_calls();

    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "go".into(),
        parts: Vec::new(),
    });

    // 另起一个任务当"用户"：等工具真跑进循环（started>0）再按会话停止。
    // 有界等待 + 到点仍然去点停止（不静默跳过）——否则保护网本身会伪装成通过。
    let waiter = {
        let active = Arc::clone(&active);
        let started = Arc::clone(&started);
        tokio::spawn(async move {
            for _ in 0..2_000 {
                if started.load(Ordering::Relaxed) > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            assert!(
                started.load(Ordering::Relaxed) > 0,
                "替身工具从没开始跑 ⇒ 这次取消打在空处，用例没有测到任何东西"
            );
            active.cancel_session("s1")
        })
    };

    let outcome = kernel.step_as(&mut session, "s1").await.unwrap();
    assert!(
        matches!(outcome, StepOutcome::Continue { .. }),
        "第一步应仍是 Continue，实测 {outcome:?}"
    );
    // ① 在途时确实有一位，且置位的就是它
    let cancelled = waiter.await.unwrap();
    assert_eq!(cancelled, 1, "停止时该会话应有一条在途调用");

    // ①′ 工具确实早停了（跑满的话 round 会是 200 且 early_stop=false）
    let out =
        tool_result_output(&session, "call-1").expect("ToolResult 必须闭合，不留悬空（纪律 5）");
    assert_eq!(out["early_stop"], json!(true), "实测输出 {out}");
    assert!(
        out["round"].as_u64().unwrap_or(u64::MAX) < 200,
        "跑满了 200 轮 = 取消没送达，实测 {out}"
    );
    assert_eq!(tool.started.load(Ordering::Relaxed), 1);

    // ② 收束后表里没位了：RAII 注销真的发生（漏一次的话这里会是 1，
    //    而下一次停止会谎报"置位了 1 条"）
    assert_eq!(active.cancel_session("s1"), 0, "调用收束后登记表必须已清空");

    // 第二轮照常收尾（取消不影响这一步之后的正常执行）
    let outcome2 = kernel.step_as(&mut session, "s1").await.unwrap();
    assert!(
        matches!(outcome2, StepOutcome::Continue { .. }),
        "实测 {outcome2:?}"
    );
}

#[tokio::test]
async fn registry_is_scoped_to_one_session() {
    let active = Arc::new(ActiveCalls::default());
    let a = ctx_with(Some("s1"), Some("call-a"));
    let b = ctx_with(Some("s2"), Some("call-b"));
    let _ga = active.track(&a).expect("带会话身份应登记");
    let _gb = active.track(&b).expect("带会话身份应登记");

    // 停止 s1 不许动到 s2 的调用：那等于替别的会话做一个不可逆决定
    assert_eq!(active.cancel_session("s1"), 1);
    assert!(a.is_cancelled());
    assert!(
        !b.is_cancelled(),
        "按会话取消越界了——动了别的会话的在途调用"
    );

    // 守卫丢弃 = 注销（这里靠"再点一次给 0"来证，不数私有字段）
    drop(_ga);
    assert_eq!(active.cancel_session("s1"), 0);
    drop(_gb);
    assert_eq!(active.cancel_session("s2"), 0);
}

#[tokio::test]
async fn registry_skips_calls_without_session_or_call_id() {
    let active = Arc::new(ActiveCalls::default());
    // 无会话身份（headless 的普通 step）⇒ 不登记：没有任何一条现成通路会去取消它，
    // 收进来只会让表里多一堆没人按的条目，把"置位了几条"这个读数稀释掉。
    assert!(active.track(&ctx_with(None, Some("call-x"))).is_none());
    // 无 call_id（第三方 ToolSource 自造的 ctx）⇒ 不登记：键的一半不在，注销没法对上。
    assert!(active.track(&ctx_with(Some("s1"), None)).is_none());
    assert_eq!(active.cancel_session("s1"), 0);
}

/// 登记序号在位的那条不变量。**现在没有产生重叠登记的生产通路**（链上一次只跑一条），
/// 这条买的是"如果哪天有了"：同键覆盖 + 先收束那条注销 = 后一条从表里消失，
/// 于是它再也收不到取消，而置位条数这个读数会少报一位、不报错。
/// 去掉 `Key.seq` 这一发就会红（本文件的变异靶）。
#[tokio::test]
async fn overlapping_registrations_of_one_call_id_do_not_erase_each_other() {
    let active = Arc::new(ActiveCalls::default());
    // 同一 (session, call_id) 登记两次（重试复用 call_id = 纪律 7 的幂等前提）
    let first = ctx_with(Some("s1"), Some("same-id"));
    let g1 = active.track(&first).expect("应登记");
    let second = ctx_with(Some("s1"), Some("same-id"));
    let g2 = active.track(&second).expect("应登记");
    assert_eq!(
        active.cancel_session("s1"),
        2,
        "两条各占一位：序号不参与键的话这里会是 1（后一条覆盖了前一条）"
    );
    // 后一条先收束：不许把还在跑的前一条一起抹掉
    drop(g2);
    assert_eq!(active.cancel_session("s1"), 1, "第一条应仍在表里可被取消");
    assert!(first.is_cancelled());
    drop(g1);
    assert_eq!(active.cancel_session("s1"), 0);
}
