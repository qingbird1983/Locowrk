//! 会话事件定义（DESIGN.md §4.1）。类型化、可序列化、只追加。

use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use plugin_api::{CallId, SideEffect};

use crate::content::ContentPart;

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

/// 用户对 uncertain 调用的裁决（G2，可靠性稿 §5.4 收窄）：原稿四选里的
/// Rerun 不设——「未执行」裁决后模型在下一轮自然重做，走全部既有审批 /
/// 守卫门，不需要旁路重执行机构。措辞用「确认」不用「跳过」：裁决记录的
/// 是用户对实际效果的事实认定，不是回避。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertainDecision {
    /// 用户确认效果已发生。
    Completed,
    /// 用户确认效果未发生（模型可安全重做）。
    NotExecuted,
    /// 用户实地检查过，[`SessionEvent::ToolExecutionUncertain`] 的
    /// `note` 携带检查记录。
    Inspected,
}

impl UncertainDecision {
    /// 事件投影 / IPC 面的字符串形态（与 serde 标签一致）。
    pub fn as_str(&self) -> &'static str {
        match self {
            UncertainDecision::Completed => "completed",
            UncertainDecision::NotExecuted => "not_executed",
            UncertainDecision::Inspected => "inspected",
        }
    }

    /// IPC 面解析（前端传 serde 标签）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "completed" => Some(UncertainDecision::Completed),
            "not_executed" => Some(UncertainDecision::NotExecuted),
            "inspected" => Some(UncertainDecision::Inspected),
            _ => None,
        }
    }
}

/// 会话事件（§4.1 全集）。`tag = "type"` + snake_case，日志即 schema。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    TurnStart {
        turn_id: TurnId,
        ts: i64,
    },
    /// `parts`（批 D，serde default）= 附件部件（图片/PDF）；旧日志缺字段
    /// 读入 = 空 = 纯文本消息，零迁移。derive_messages 随 User 消息携带
    /// （wire 层按需编码；压缩摘要/转录/标题只吃正文）。
    UserMessage {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<ContentPart>,
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
    /// 上下文用量快照（v2.62，§3.12 进度条数据源持久化）：轮次前与每个
    /// step 后各一条。`used` 口径（v2.138 用户裁定统一锚点）：优先供应商
    /// 真实 input（日志最后一条 LlmUsage，与整理触发/自动收起同一把尺
    /// ——「看到的 = 触发用的」）；无上报回落估算（树组装口径优先，
    /// 字符估算兜底）。derive_messages 跳过（用量是过程计量不是上下文，
    /// 不回传供应商）；投影层映成 `agent://context-usage`——重放恢复用量
    /// 环（此前只推实时通道，切会话回来统计即消失）。字段全
    /// `serde(default)`（向后兼容旧日志，ContextCompacted 同款）。
    /// 旧日志无此变体。
    ContextUsage {
        #[serde(default)]
        used: u64,
        #[serde(default)]
        threshold: u64,
        #[serde(default)]
        window_tokens: u64,
        #[serde(default)]
        tools_est_tokens: u64,
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
    /// 崩溃恢复的用户裁决（G2，可靠性稿 §5）：进程在 ToolCall 落日志
    /// （纪律 5）与 ToolResult 闭合之间退出时，该调用的副作用未知
    /// （Unknown Effect——"删了没有？"无法从本地证据判断）。扫描面
    /// [`crate::uncertain::scan_uncertain`] 把悬空调用标成 uncertain，
    /// 消息投影给模型一个如实的「效果未知」闭合；用户裁决后落本事件
    /// （只写一次，重放/重启不再重复询问）。裁决不回填 ToolResult——
    /// 日志只追加真实发生的事，闭合是投影层的合成（可随时重推导）。
    ToolExecutionUncertain {
        call_id: CallId,
        decision: UncertainDecision,
        /// inspected 裁决的用户检查记录；其余裁决为空串。
        #[serde(default)]
        note: String,
        /// 裁决时刻（Unix 毫秒）；0 = 旧日志无（serde default 兼容）。
        #[serde(default)]
        ts: i64,
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
        /// 收束时刻（Unix 毫秒，v2.101 起写入）：与 [`SessionEvent::TurnStart`]
        /// 的起点一对，重放复原轮计时行读数（「已工作 N 分 N 秒」）。
        /// 0 = 旧日志无（serde default 兼容），投影按缺失处理不虚构。
        #[serde(default)]
        ts: i64,
    },
}

#[cfg(test)]
mod tests;
