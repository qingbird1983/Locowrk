//! UI 事件出口（DESIGN.md §12.2）：`agent://` 通道 + EventSink 抽象。
//!
//! 纪律：流式输出走事件推送，不塞 command 返回值。⑳ 起真实流式：LLM
//! 文本 delta 经 `StreamChunkSink`（内核 `LlmStreamSink`）逐段直推同一
//! `agent://chunk` 通道，该步的整段投影被 runner 抑制；无 sink 的路径
//! （headless / 内核测试）回到整段投影。前端追加逻辑不变。

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
    pub const PLUGIN_STATE: &str = "plugin://state-changed";
    /// 上下文用量（⑨）：进度条数据源（used / threshold，同 estimate 口径）。
    pub const CONTEXT_USAGE: &str = "agent://context-usage";
    /// 压缩完成（⑨）：前端推 SystemNotice（[查看详情][撤销]）。
    pub const CONTEXT_COMPACTED: &str = "agent://context-compacted";
    /// 定时触发（⑭）：调度引擎向会话注入轮之前推送，前端推 SystemNotice。
    pub const SCHEDULE_FIRED: &str = "agent://schedule-fired";
    /// 审批结果（㉕·对话区① §3.17）：`ApprovalDecided` 的投影通道——前端
    /// 渲染"决策记录条"并摘除对应待决卡。
    pub const APPROVAL_RESOLVED: &str = "agent://approval-resolved";
    /// 用户消息（㉕·fix）：仅供全量投影（`session_history`）产出——实时轮
    /// 前端发送即本地回显，runner 不投影该段（防双气泡）。本通道因此
    /// **永不 emit**，前端也**有意不订阅**（api.ts CHANNELS 不含它）。
    pub const USER_MESSAGE: &str = "agent://user-message";
    /// 推理内容（v2.57 §3.9）：实时 delta 直推（StreamChunkSink）与整段
    /// 投影（runner）共用；重放经 session_history 不走通道。
    pub const THINKING: &str = "agent://thinking";
    /// 单步 LLM 用量（v2.58 §3.12）：runner 投影（实时与重放同形）。
    pub const LLM_USAGE: &str = "agent://usage";
}

/// 审批请求载荷（§13.2）：`agent://approval-request` 的事件体，
/// 同时是 `list_pending_approvals` 的返回项。字段与其他 `agent://` 事件
/// 一致用 snake_case（§13.2 示例里的 camelCase 以本实现为准）。
/// `session_id`（⑮）：发起调用的会话——子 Agent 派生的调用带
/// `{parent}-sub-{n}`，前端据此打"子 Agent"标识徽章。
/// `description`（㉕·对话区① §3.17）：工具自述（`ToolSpec.description`），
/// 决策卡理由行的数据源；空描述不带字段语义（None）。
/// `timeout_secs`（v2.49）：守夜超时秒数，决策卡倒计时的数据源；
/// 0 = 不限（设置"一直等待"档），卡片不渲染倒计时。
/// `workspace`（㉘·B §3.17）：会话真实归属——决策卡环境行"这条将在哪儿
/// 跑"的数据源（此前恒取 fsRoots[0] 是占位）；未归属 None（前端回落
/// fsRoots[0] / "临时会话"）。
/// `external`（㉘·B）：外部路径门命中——围栏外命令逐次审批、只许一次，
/// 前端据此**不渲染**「允许本次对话」。
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRequestPayload {
    pub request_id: String,
    pub tool: String,
    pub args: Value,
    pub side_effect: String,
    pub session_id: Option<String>,
    pub description: Option<String>,
    pub timeout_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub external: bool,
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
    /// 上下文用量（⑨）：每个 step 后推送；used 与压缩判定同一估算口径
    /// （序列化字节 / 4，kernel_agent::estimate_tokens）。
    /// `tools_est_tokens`（v2.58 §3.12）：本轮请求实际携带的暴露目录估算
    /// （0 = 目录缺席），工具上下文行的"约 N tokens"。
    ContextUsage {
        session_id: String,
        used: u64,
        threshold: u64,
        /// 模型窗口（未知 = 0，前端按"未设置"处理）。
        window_tokens: u64,
        #[serde(default)]
        tools_est_tokens: u64,
    },
    /// 单步 LLM 用量（v2.58 §3.12）：`SessionEvent::LlmUsage` 的投影，
    /// 实时与重放同形——前端按轮聚合出「本轮合计 / 生成速度 / 模型用量」。
    /// 供应商不报告的 step 不产生（宁缺勿假）。
    LlmUsage {
        session_id: String,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
        reasoning: u64,
        total: u64,
        duration_ms: u64,
    },
    /// 压缩完成（⑨）：summary 只推 200 字符预览（全文在会话日志）。
    ContextCompacted {
        session_id: String,
        from_tokens: u64,
        to_tokens: u64,
        /// sliding_window / layered / summary_only（文案前端渲染）。
        strategy: String,
        summary_preview: String,
    },
    /// 定时触发（⑭）：调度引擎注入轮之前告知——SystemNotice 无动作按钮，
    /// 后续助手回复以 chunk 跟进；窗口不在位时后端另有系统通知兜底。
    ScheduleFired {
        session_id: String,
        schedule_id: String,
        title: String,
    },
    /// 用户消息（㉕·fix）：只在会话日志全量投影出现，重放恢复用户气泡；
    /// 实时轮不投影（前端本地回显），见 channels::USER_MESSAGE 注释。
    /// `pos`（v2.57 §3.34）：源事件在日志里的位次——消息编辑重发（截断）
    /// 的定位键；实时路径不产出（前端本地回显无 pos）。
    UserMessage {
        session_id: String,
        content: String,
        pos: usize,
    },
    /// 推理内容（v2.57 §3.9）：`SessionEvent::Thinking` 的投影。实时与
    /// 重放**同一事件形状**：实时 delta 直推（`text` = 增量，StreamChunkSink
    /// on_reasoning）与整段投影（`text` = 全文，已被流式消费的 step 抑制）
    /// 都走 append-or-create；重放每条一行。
    Thinking {
        session_id: String,
        text: String,
    },
    /// 审批结果（㉕·对话区① §3.17）：`SessionEvent::ApprovalDecided` 的
    /// 投影（runner），实时增量与历史重放同一来源——前端渲染"决策记录条"
    /// （已允许/已拒绝 · 动作名）并摘除待决卡；`tool` 由 runner 从同切片
    /// 的 `ToolCall` 按 call_id 解析（审批决策卡按它找回宿主工具行）。
    ApprovalResolved {
        session_id: String,
        call_id: String,
        tool: String,
        allowed: bool,
        reason: Option<String>,
    },
}

impl AgentEvent {
    pub fn channel(&self) -> &'static str {
        match self {
            AgentEvent::Chunk { .. } => channels::CHUNK,
            AgentEvent::ToolCall { .. } => channels::TOOL_CALL,
            AgentEvent::ToolResult { .. } => channels::TOOL_RESULT,
            AgentEvent::TurnDone { .. } => channels::TURN_DONE,
            AgentEvent::ApprovalRequest(_) => channels::APPROVAL_REQUEST,
            AgentEvent::ContextUsage { .. } => channels::CONTEXT_USAGE,
            AgentEvent::ContextCompacted { .. } => channels::CONTEXT_COMPACTED,
            AgentEvent::ScheduleFired { .. } => channels::SCHEDULE_FIRED,
            AgentEvent::UserMessage { .. } => channels::USER_MESSAGE,
            AgentEvent::ApprovalResolved { .. } => channels::APPROVAL_RESOLVED,
            AgentEvent::Thinking { .. } => channels::THINKING,
            AgentEvent::LlmUsage { .. } => channels::LLM_USAGE,
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

/// 插件状态广播转发（§13.4）：`plugin://state-changed` 的载荷就是
/// plugin-service `PluginStateChange` 的序列化形态（自带 `type` 标签），
/// 不经 AgentEvent 包装。这是组装点胶水——编排层不依赖 Tauri。
pub fn forward_plugin_states(
    app: AppHandle,
    mut rx: tokio::sync::broadcast::Receiver<plugin_service::PluginStateChange>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(change) => {
                    if let Err(e) = app.emit(channels::PLUGIN_STATE, &change) {
                        tracing::warn!("emit {} failed: {e}", channels::PLUGIN_STATE);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("plugin state broadcast lagged, {n} changes skipped");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
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
        // §3.13 应用内通知（v2.51）：五类「用户可能错过的事」在唯一事件
        // 出口处统一入账（提取→落 store→推 lw://notification），ApprovalHub
        // 与调度引擎零改动。
        crate::notifications::record_event(&self.0, event);
    }
}

/// ⑳ LLM 文本 delta 直推（`LlmStreamSink` 的桌面实现）：与整段投影共用
/// `agent://chunk` 通道与追加语义——流式步骤的整段投影被 runner 抑制
/// （StepOutcome::streamed），前端无需区分。带会话身份的 step 才发
/// （headless / 无身份 step 无 UI 可推）。子 Agent 的 delta 已在内核侧
/// 被 child_kernel 的 NoLlmStreamSink 压制（黑盒不泄漏）。
pub struct StreamChunkSink(AppHandle);

impl StreamChunkSink {
    pub fn new(app: AppHandle) -> Self {
        Self(app)
    }
}

impl kernel_agent::LlmStreamSink for StreamChunkSink {
    fn on_delta(&self, session_id: Option<&str>, delta: &str) -> bool {
        let Some(session_id) = session_id else {
            return false;
        };
        if delta.is_empty() {
            return false;
        }
        let event = AgentEvent::Chunk {
            session_id: session_id.into(),
            delta: delta.into(),
        };
        match self.0.emit(event.channel(), &event) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("emit {} failed: {e}", event.channel());
                false
            }
        }
    }

    /// 推理 delta（v2.57）：与正文同形直推 `agent://thinking`，语义与
    /// on_delta 一致（消费成功 = 该步整段 Thinking 投影被抑制）。子会话
    /// 的推理同样被 child_kernel 的 NoLlmStreamSink 压制（黑盒不泄漏）。
    fn on_reasoning(&self, session_id: Option<&str>, delta: &str) -> bool {
        let Some(session_id) = session_id else {
            return false;
        };
        if delta.is_empty() {
            return false;
        }
        let event = AgentEvent::Thinking {
            session_id: session_id.into(),
            text: delta.into(),
        };
        match self.0.emit(event.channel(), &event) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("emit {} failed: {e}", event.channel());
                false
            }
        }
    }
}
