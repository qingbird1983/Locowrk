//! UI 事件出口（DESIGN.md §12.2）：`agent://` 通道 + EventSink 抽象。
//!
//! 纪律：流式输出走事件推送，不塞 command 返回值。阶段五的 LLM 非流式，
//! 每步正文作为一整块 `agent://chunk` 发出；真实模型接入后同一通道只是
//! delta 变小，前端累积逻辑不变。

use plugin_api::SideEffect;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

pub mod channels {
    pub const CHUNK: &str = "agent://chunk";
    pub const TOOL_CALL: &str = "agent://tool-call";
    pub const TOOL_RESULT: &str = "agent://tool-result";
    pub const TURN_DONE: &str = "agent://turn-done";
    pub const APPROVAL_REQUEST: &str = "agent://approval-request";
}

/// 审批请求载荷（§13.2）：`agent://approval-request` 的事件体，
/// 同时是 `list_pending_approvals` 的返回项。字段与其他 `agent://` 事件
/// 一致用 snake_case（§13.2 示例里的 camelCase 以本实现为准）。
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRequestPayload {
    pub request_id: String,
    pub tool: String,
    pub args: Value,
    pub side_effect: String,
}

/// 推给前端的事件。字段命名与 session-store 事件对齐（snake_case）；
/// 前端镜像类型在 `desktop/src/types.ts`。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    Chunk {
        session_id: String,
        delta: String,
    },
    ToolCall {
        session_id: String,
        call_id: String,
        tool: String,
        args: Value,
        side_effect: String,
        plugin_version: String,
    },
    ToolResult {
        session_id: String,
        call_id: String,
        ok: bool,
        output: Value,
        error: Option<String>,
    },
    TurnDone {
        session_id: String,
        steps: u32,
    },
    ApprovalRequest(ApprovalRequestPayload),
}

impl AgentEvent {
    pub fn channel(&self) -> &'static str {
        match self {
            AgentEvent::Chunk { .. } => channels::CHUNK,
            AgentEvent::ToolCall { .. } => channels::TOOL_CALL,
            AgentEvent::ToolResult { .. } => channels::TOOL_RESULT,
            AgentEvent::TurnDone { .. } => channels::TURN_DONE,
            AgentEvent::ApprovalRequest(_) => channels::APPROVAL_REQUEST,
        }
    }
}

/// SideEffect 的事件载荷表示（runner 投影与审批请求共用，保持单一来源）。
pub(crate) fn side_effect_str(se: &SideEffect) -> &'static str {
    match se {
        SideEffect::Pure => "pure",
        SideEffect::Compensable { .. } => "compensable",
        SideEffect::Irreversible => "irreversible",
    }
}

/// 事件出口抽象：runner 只认它，测试用收集器替换 TauriSink（§10.3 分层）。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &AgentEvent);
}

/// Tauri 实现：emit 到全局（单窗口阶段足够；多窗口再换 Channel）。
pub struct TauriSink(AppHandle);

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self(app)
    }
}

impl EventSink for TauriSink {
    fn emit(&self, event: &AgentEvent) {
        if let Err(e) = self.0.emit(event.channel(), event) {
            tracing::warn!("emit {} failed: {e}", event.channel());
        }
    }
}
