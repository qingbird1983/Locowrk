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
    TurnEnd {
        turn_id: TurnId,
        outcome: TurnOutcome,
    },
}
