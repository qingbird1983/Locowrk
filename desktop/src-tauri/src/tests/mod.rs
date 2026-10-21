//! 桌面端单元测试：runner 的日志投影循环 / demo_llm 决策规则 / 步数上限收束。
//! （plugins_root 工件发现用例在 tests_plugins_root.rs；审批门用例在
//! tests_approval.rs——400 行纪律拆出。事件出口用收集器替换 TauriSink。）
//! 批 F-17 目录化：共享假件（进程内 echo 工具 / 事件收集器 / 内核装配）留本文件，
//! 用例按面分簇见 `turn_rounds` / `demo_routing` / `convergence`。
//! S2 刀④ 加 `compact_wiring`：同一形状的**主层压缩**接线判据（含落盘同数那一半）。
//! specs/12 刀② 加 `compact_status_wiring`：压缩的**界面反馈与预算**接线判据；
//! specs/13 加 `compaction_opencode`：AC-1…AC-10 的验收判据族（提示词逐字 / 门槛 /
//! 请求形态 / 保留区 / 三种触发 / 失败处置 / 收缩矩阵 / 记账）。
//! 两簇接线面共用**落盘夹具**（`seed_log`：把内存会话的前史先写进磁盘）⇒ 因此上移
//! 本文件——共用夹具住共用夹具模块（400 行纪律的正解是「拆」，不是「把行数摊到别处」）。

mod compact_status_wiring;
mod compact_wiring;
mod compaction_opencode;
mod convergence;
mod demo_routing;
mod turn_rounds;

use crate::demo_llm::DemoLlm;
use crate::events::{AgentEvent, EventSink};
use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{Kernel, LlmService, ToolSource};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};
use session_store::{Session, SessionEvent};
use std::path::Path;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

/// 进程内 echo 工具（不依赖插件产物）。
struct EchoTool;

fn echo_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "echo".into(),
        description: "test echo".into(),
        schema: json!({ "type": "object" }),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    })
}

#[async_trait]
impl Tool for EchoTool {
    fn spec(&self) -> &ToolSpec {
        echo_spec()
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(1, 0, 0);
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        Ok(json!({
            "echo": args.get("text").and_then(Value::as_str).unwrap_or(""),
        }))
    }
}

struct SingleToolSource(Arc<EchoTool>);

#[async_trait]
impl ToolSource for SingleToolSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        _name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        Ok(Arc::clone(&self.0) as Arc<dyn Tool>)
    }

    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        vec![echo_spec().clone()]
    }
}

/// 测试事件收集器。
#[derive(Clone, Default)]
pub(crate) struct Collector(Arc<StdMutex<Vec<AgentEvent>>>);

impl Collector {
    pub(crate) fn events(&self) -> Vec<AgentEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl EventSink for Collector {
    fn emit(&self, event: &AgentEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

pub(crate) fn kind(e: &AgentEvent) -> &'static str {
    match e {
        AgentEvent::Chunk { .. } => "chunk",
        AgentEvent::ToolCall { .. } => "tool_call",
        AgentEvent::ToolResult { .. } => "tool_result",
        AgentEvent::TurnDone { .. } => "turn_done",
        AgentEvent::ApprovalRequest(_) => "approval_request",
        AgentEvent::ContextUsage { .. } => "context_usage",
        AgentEvent::ContextCompacted { .. } => "context_compacted",
        AgentEvent::ScheduleFired { .. } => "schedule_fired",
        AgentEvent::UserMessage { .. } => "user_message",
        AgentEvent::ApprovalResolved { .. } => "approval_resolved",
        AgentEvent::Thinking { .. } => "thinking",
        AgentEvent::LlmUsage { .. } => "llm_usage",
        AgentEvent::AskUser { .. } => "ask_user",
        AgentEvent::AskResolved { .. } => "ask_resolved",
        AgentEvent::LlmRetry { .. } => "llm_retry",
        AgentEvent::TurnCapped { .. } => "turn_capped",
        AgentEvent::ToolUncertain { .. } => "tool_uncertain",
        AgentEvent::UncertainResolved { .. } => "uncertain_resolved",
        AgentEvent::ToolStatus { .. } => "tool_status",
        AgentEvent::FileChanges { .. } => "file_changes",
        AgentEvent::TurnHeader { .. } => "turn_header",
        AgentEvent::CompactStatus { .. } => "compact_status",
    }
}

pub(crate) fn kernel_with_tools() -> Kernel {
    kernel_with_tools_streaming(None)
}

/// 带可选 LlmStreamSink 的同型内核（⑳ 流式抑制测试用）。
pub(crate) fn kernel_with_tools_streaming(
    sink: Option<Arc<dyn kernel_agent::LlmStreamSink>>,
) -> Kernel {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(SingleToolSource(Arc::new(EchoTool))));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(DemoLlm::new()));
    if let Some(sink) = sink {
        ctx.provide_dyn::<dyn kernel_agent::LlmStreamSink>(sink);
    }
    Kernel::new(ctx, vec![])
}

// ---------- specs/12 刀②：压缩两簇接线面共用的会话夹具 ----------
//
// 为什么上移本文件：压缩的**落账接线面**（`compact_wiring`）与压缩的**界面反馈与
// 预算接线面**（`compact_status_wiring`）要同一份"到线了"的会话。夹具住在其中
// 一个文件里，另一个就得 `use super::compact_wiring::…`——那是"为让另一个文件用上
// 而把夹具钉死在它旁边"。共用夹具住共用夹具模块（本文件已经是这个角色：
// 收集器 / 事件种类 / 内核装配都在这儿）。

/// 先把内存会话的**前史**写进磁盘。
///
/// `run_turn` 只追加本轮新增段（磁盘是日志的镜像、与投影共用游标），而夹具里的会话
/// 是内存里造的 ⇒ 不落这一次，磁盘上就没有前史。而折叠是**按投影里有多少条消息**算
/// 的（`kept_messages` 与投影条数取小），缺前史的重放会得出另一个窗口——那是夹具的
/// 缺陷，不是压缩层的性质，所以先把地基铺好再谈取证。
pub(crate) fn seed_log(log: &Path, id: &str, session: &Session) {
    crate::session_log::append_slice(log, id, session, 0);
}

/// 会话里落了几次压缩（**内存会话**与**磁盘镜像**两处都要数 ⇒ 共用这一颗）。
///
/// 为什么上移本文件：压缩落盘是**两个触发路径各做各的**（自动那条在 `runner`
/// 的 step 边界、手动那条在压缩层的 `run_manual_and_persist`），两处都要用同一
/// 把尺数「磁盘镜像里那条压缩事件在不在」——尺住在其中一个文件里，另一个就得
/// `use super::compact_wiring::…` ⇒ 那是"为让另一个文件用上而把夹具钉死在它旁边"。
pub(crate) fn compacted_count(session: &Session) -> usize {
    session
        .events()
        .iter()
        .filter(|e| matches!(e, session_store::SessionEvent::ContextCompacted { .. }))
        .count()
}
