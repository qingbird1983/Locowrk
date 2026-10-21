//! specs/13 的验收判据（AC-1 … AC-13）：上下文压缩按 OpenCode V2 重写。
//!
//! 分六个子文件（400 行纪律）：
//! · `prompt`   —— 提示词逐字（AC-3）与输出校验（AC-4）
//! · `window`   —— 门槛（AC-1）、请求形态（AC-2）、保留区（AC-6）、三种触发（AC-7）
//! · `checkpoint`—— checkpoint 逐字节（AC-5）
//! · `failure`  —— 失败处置（AC-8）、收缩矩阵（AC-9）、失败也记账（AC-10）
//! · `persist`  —— 手动那一趟落盘（AC-13）
//! · `event`    —— 压成事件带摘要**全文**（AC-18，刀⑤）
//!
//! 共享夹具住本文件：**假发送口**（脚本化，可回话也可报错）、**会话夹具**
//! （占用读数已知）、**策略装配**（窗口 + 用户百分比）。

mod checkpoint;
mod event;
mod failure;
mod persist;
mod prompt;
mod window;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use kernel_agent::{Kernel, LlmError, LlmRequest, LlmResponse, LlmService, LlmUsage};
use session_store::{Message, Session, SessionEvent};

use crate::compact::deep::{CompactionLlm, Compactor, Hooks};
use crate::runner::UsagePolicy;

/// 一句回话的结局。
#[derive(Clone)]
pub(crate) enum Reply {
    /// 回这一段正文（当作模型的摘要）。
    Text(String),
    /// 报这一句错。
    Fail(String),
}

/// 脚本化的压缩发送口：**数趟数**、**把每一次请求原样留下来**。
///
/// 为什么要有它：压缩请求的真实出口是网络，而这一族要钉的是"发出去的请求长
/// 什么样"与"模型回了什么" ⇒ 必须能在**请求层**观察，不能只看最终结果。
pub(crate) struct Script {
    replies: Vec<Reply>,
    seen: StdMutex<Vec<LlmRequest>>,
    calls: AtomicUsize,
}

impl Script {
    /// 造一个脚本。脚本用完时**重复最后一条**（⇒ 单一结局的脚本可以一直试）。
    pub(crate) fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Script {
            replies,
            seen: StdMutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
        })
    }

    /// 发出去过几趟。
    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// 逐趟的请求（按发送顺序）。
    pub(crate) fn requests(&self) -> Vec<LlmRequest> {
        self.seen.lock().expect("script seen poisoned").clone()
    }
}

#[async_trait]
impl CompactionLlm for Script {
    async fn complete(
        &self,
        req: LlmRequest,
        _budget: Duration,
        _on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().expect("script seen poisoned").push(req);
        let reply = self
            .replies
            .get(n)
            .or_else(|| self.replies.last())
            .cloned()
            .unwrap_or(Reply::Fail("script empty".into()));
        match reply {
            Reply::Text(t) => Ok(LlmResponse {
                content: t,
                tool_calls: Vec::new(),
                usage: Some(LlmUsage {
                    input: 111,
                    output: 222,
                    cache_read: 0,
                    cache_write: 0,
                    reasoning: 0,
                    total: 333,
                }),
            }),
            Reply::Fail(e) => Err(LlmError::Backend(e)),
        }
    }
}

/// 造一台压缩器（设置 + 内核 + 假发送口）。
///
/// `enabled` = 自动压缩开关（手动那一格要测"关着也能压"）；`pct` = 用户那一格。
pub(crate) fn compactor(
    tag: &str,
    script: Arc<Script>,
    enabled: bool,
    pct: u64,
) -> (Arc<Compactor>, std::path::PathBuf) {
    compactor_with(
        tag,
        script,
        enabled,
        pct,
        Arc::new(crate::tests::kernel_with_tools()),
    )
}

/// 造一台压缩器（内核由调用方给——AC-2 要一台**带请求决策环捕获**的）。
pub(crate) fn compactor_with(
    tag: &str,
    script: Arc<Script>,
    enabled: bool,
    pct: u64,
    kernel: Arc<Kernel>,
) -> (Arc<Compactor>, std::path::PathBuf) {
    let root = crate::testutil::work_dir(tag);
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({
            "context": { "compactEnabled": enabled, "autoCompactPct": pct }
        })
        .to_string(),
    )
    .expect("写测试设置失败");
    let settings = Arc::new(crate::settings::SettingsStore::open(&root).expect("设置库打开失败"));
    let cp = Arc::new(Compactor::new(
        settings,
        kernel,
        script as Arc<dyn CompactionLlm>,
        Arc::new(crate::compact::Failures::default()),
        Arc::new(crate::compact::Overflow::default()),
        None,
    ));
    (cp, root)
}

/// 一台**捕获 `agent/request` 决策环载荷**的内核。
///
/// 为什么需要：AC-2 要证"hooks 看到的是**加提示词前**的请求"——那件事只发生在
/// 决策环里，事后从发出去的请求反推是证不出来的。
pub(crate) fn kernel_capturing_requests(seen: Arc<StdMutex<Vec<Vec<Message>>>>) -> Kernel {
    struct Capture(Arc<StdMutex<Vec<Vec<Message>>>>);
    #[async_trait]
    impl cordis_core::WaterfallListener for Capture {
        async fn handle(
            &self,
            _key: &str,
            payload: &mut serde_json::Value,
        ) -> cordis_core::WaterfallDecision {
            let msgs: Vec<Message> =
                serde_json::from_value(payload.get("messages").cloned().unwrap_or_default())
                    .unwrap_or_default();
            self.0.lock().expect("capture poisoned").push(msgs);
            cordis_core::WaterfallDecision::Continue
        }
    }
    let ctx = Arc::new(cordis_core::Context::new());
    ctx.provide_dyn::<dyn kernel_agent::ToolSource>(Arc::new(crate::tests::SingleToolSource(
        Arc::new(crate::tests::EchoTool),
    )));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(crate::demo_llm::DemoLlm::new()));
    ctx.events
        .on_waterfall("agent/request", Arc::new(Capture(seen)));
    Kernel::new(ctx, vec![])
}

/// 一个"占用已知"的会话：`rounds` 轮对话 + 一条用量上报（占用 = `input`）。
///
/// 取数纪律：**不钉魔法数**——占用由那条上报给出，于是"到线了没"这件事由夹具
/// 自己说了算。
pub(crate) fn session_with_usage(input: u64, rounds: usize, size: usize) -> Session {
    let mut s = Session::new();
    for i in 0..rounds {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题：{}", "细节".repeat(size)),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮答复"),
            tool_calls: Vec::new(),
        });
    }
    s.append(SessionEvent::LlmUsage {
        input,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: input + 10,
        duration_ms: 1,
    });
    s
}

/// 门槛策略：窗口已知（token）+ 用户百分比。
pub(crate) fn policy(window: u64, pct: u64) -> UsagePolicy {
    UsagePolicy::assemble(pct, Some(window), None, 0, 0)
}

/// 门槛策略：窗口**未知**（自动那一档因此不触发，同上游 A2）。
pub(crate) fn policy_unknown(pct: u64) -> UsagePolicy {
    UsagePolicy::assemble(pct, None, None, 0, 0)
}

/// 空钩子（这一簇不测界面事件）。
pub(crate) fn hooks() -> Hooks<'static> {
    static ON_START: fn() = || {};
    static ON_DELTA: fn(&str) = |_| {};
    Hooks {
        on_start: &ON_START,
        on_delta: &ON_DELTA,
    }
}

/// 一份**合格**的摘要（带模板标题行 ⇒ 输出校验通过）。
pub(crate) fn valid_summary() -> String {
    "## Objective\n- 把上下文压缩按 OpenCode V2 重写\n\n\
     ## Requirements\n- (none)\n\n\
     ## Decisions\n- (none)\n\n\
     ## Work State\n### Completed\n- 压缩面已重写\n\n\
     ### Active\n- (none)\n\n\
     ### Blocked\n- (none)\n\n\
     ## Next Move\n1. (none)\n\n\
     ## Relevant Files\n- (none)\n\n\
     ## Important Context\n- (none)"
        .to_string()
}

/// 一台"每次都报错"的内核（错误文本可配）。
pub(crate) fn failing_kernel(msg: &str) -> Kernel {
    struct Failing(String);
    #[async_trait]
    impl LlmService for Failing {
        async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
            Err(LlmError::Backend(self.0.clone()))
        }
    }
    let ctx = Arc::new(cordis_core::Context::new());
    ctx.provide_dyn::<dyn LlmService>(Arc::new(Failing(msg.to_string())));
    Kernel::new(ctx, vec![])
}

/// 消息正文（取 user 消息的正文；别的是 user 消息就当场报错）。
pub(crate) fn user_text(m: &Message) -> &str {
    match m {
        Message::User { content, .. } => content,
        other => panic!("期望一条 user 消息，实得 {other:?}"),
    }
}

/// user 消息的正文（**不是** user 消息就当没有）——扫一列消息时用。
pub(crate) fn maybe_user_text(m: &Message) -> Option<&str> {
    match m {
        Message::User { content, .. } => Some(content),
        _ => None,
    }
}
