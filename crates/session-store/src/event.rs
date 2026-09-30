//! 会话事件定义（DESIGN.md §4.1）。类型化、可序列化、只追加。

use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use plugin_api::{CallId, SideEffect};

/// 轮次标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnId(pub u64);

/// 轮次结局。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnOutcome {
    Completed,
    Failed,
}

/// 会话事件（§4.1 全集）。`tag = "type"` + snake_case，日志即 schema。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    TurnStart {
        turn_id: TurnId,
        ts: i64,
    },
    UserMessage {
        content: String,
    },
    /// 只存 CallId；工具名与参数从 ToolCall 事件回填（§4.2）。
    AssistantMessage {
        content: String,
        tool_calls: Vec<CallId>,
    },
    /// 先写日志再执行（纪律 5）：崩溃后 replay 能识别悬空调用。
    ToolCall {
        call_id: CallId,
        tool: String,
        args: Value,
        plugin_version: Version,
        side_effect: SideEffect,
    },
    ToolResult {
        call_id: CallId,
        output: Value,
        error: Option<String>,
    },
    /// effect_id 与 cordis-core::EffectId 的原始值对齐
    /// （session-store 只依赖 plugin-api，不依赖 cordis-core，§8）。
    EffectApplied {
        effect_id: u64,
        description: String,
    },
    PluginActivated {
        plugin_id: String,
        from: Option<Version>,
        to: Version,
    },
    ManualIntervention {
        call_id: CallId,
        reason: String,
    },
    /// 审批决策本身也是事件（纪律 9）。
    ApprovalDecided {
        call_id: CallId,
        allowed: bool,
        reason: Option<String>,
    },
    /// 上下文压缩（Context and Memory.md §3.7）：ctx-compact 在
    /// `agent/request` 决策环完成压缩后由 kernel 落日志。折叠语义
    /// （session.rs）：把已累计消息留尾部 `kept_messages` 条，其余替换为
    /// 一条摘要 User 消息；撤销 = rewind_to(本事件序号)（前缀游标，
    /// 其后的轮次一并回退）。字段全 `serde(default)`：后续演进向后兼容
    /// 旧日志。摘要存全文（折叠重建需要）；UI 投影另截前 200 字符。
    ContextCompacted {
        #[serde(default)]
        kept_messages: usize,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        from_tokens: u64,
        #[serde(default)]
        to_tokens: u64,
        /// sliding_window / layered / summary_only（文案由前端按 locale 渲染）。
        #[serde(default)]
        strategy: String,
        #[serde(default)]
        ts: i64,
    },
    TurnEnd {
        turn_id: TurnId,
        outcome: TurnOutcome,
    },
}
