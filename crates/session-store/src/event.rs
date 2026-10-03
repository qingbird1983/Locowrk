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
    /// 推理内容（v2.57）：供应商 reasoning 全文，每个 LLM step 至多一条、
    /// 紧贴其 AssistantMessage 之前。derive_messages 跳过（推理是过程不是
    /// 上下文：不回传供应商，压缩折叠与用量估算不受影响）；投影层把它映成
    /// §3.9 思考行。旧日志无此变体（serde 天然兼容）。
    Thinking {
        text: String,
    },
    /// 只存 CallId；工具名与参数从 ToolCall 事件回填（§4.2）。
    AssistantMessage {
        content: String,
        tool_calls: Vec<CallId>,
    },
    /// 单步 LLM 用量（v2.58，DESIGN §4.1）：供应商报告的真实 token 计数与
    /// 耗时，每个 LLM step 至多一条、紧贴其 AssistantMessage 之后。
    /// derive_messages 跳过（用量是过程计量不是上下文，不回传供应商）；
    /// 投影层把它映成 §3.12 模型用量明细。供应商不报告 = 不落事件
    /// （宁缺勿假，不用估算冒充真实计数）。旧日志无此变体（serde 天然兼容）。
    LlmUsage {
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
        reasoning: u64,
        total: u64,
        duration_ms: u64,
    },
    /// 先写日志再执行（纪律 5）：崩溃后 replay 能识别悬空调用。
    /// `plugin_id` 是 ⑩ 使用统计的归属键；内置替身 / 旧日志缺省 None
    /// （serde default 兼容，exposure 文档 §3.4）。
    ToolCall {
        call_id: CallId,
        tool: String,
        args: Value,
        #[serde(default)]
        plugin_id: Option<String>,
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

#[cfg(test)]
mod tests;
