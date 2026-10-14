//! UI 事件出口（DESIGN.md §12.2）：`agent://` 通道 + EventSink 抽象。
//!
//! 纪律：流式输出走事件推送，不塞 command 返回值。⑳ 起真实流式：LLM
//! 文本 delta 经 `StreamChunkSink`（内核 `LlmStreamSink`）逐段直推同一
//! `agent://chunk` 通道，该步的整段投影被 runner 抑制；无 sink 的路径
//! （headless / 内核测试）回到整段投影。前端追加逻辑不变。

use std::sync::Arc;

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
    /// 任务选择请求（v4.11 §3.39①）：`ask_user` 工具反向请求——前端渲染
    /// composer 坞内选择卡，应答经 `ask_respond` 回填。
    pub const ASK_USER: &str = "agent://ask-user";
    /// 选择收束（v4.11 §3.39①）：应答/跳过/超时/停止四路同源——前端摘卡。
    pub const ASK_RESOLVED: &str = "agent://ask-resolved";
    /// 传输重试（LLM.md v3.0）：兼容客户端重试前推——前端重置流式缓冲。
    pub const LLM_RETRY: &str = "agent://llm-retry";
    /// 单轮步数到顶（LLM.md v3.0 同批）：TurnDone 后如实告知（不硬报错）。
    pub const TURN_CAPPED: &str = "agent://turn-capped";
    /// 崩溃恢复 uncertain（G2，可靠性稿 §5）：全量投影里悬空调用的标记
    /// ——前端渲染 uncertain 决策卡 + 工具行「结果未知」态。只在会话
    /// 重放产出（悬空只在崩溃后出现），本通道**永不 emit**（USER_MESSAGE
    /// 同款纪律）；裁决经 `agent://uncertain-resolved` 实时回推。
    pub const TOOL_UNCERTAIN: &str = "agent://tool-uncertain";
    /// 用户裁决落日志后的实时回推（G2）：前端收束决策卡、按裁决闭合
    /// 工具行（随后的合成 ToolResult 投影同一来源闭合行明细）。
    pub const UNCERTAIN_RESOLVED: &str = "agent://uncertain-resolved";
    /// 整理段落总结（Organize Summary Design v1 §3.3）：轮开场排空落日志
    /// 后实时回推（与重放投影同形，uncertain-resolved 同款纪律）。前端渲染
    /// 淡色总结块（断层交接：此前工作段的浓缩叙事）。
    pub const CONTEXT_SUMMARY: &str = "agent://context-summary";
    /// 工具行状态（CF §2.1，队列 Q12·C1）：**只在实时路径 emit，不进会话日志**
    /// （ApprovalRequest 同款纪律）。现役唯一取值 = `pending_approval`——行的
    /// 其余状态由 `ToolCall` / `ToolResult` / 裁决事件自己推出，一份事实只许
    /// 一个出处，不在这里重复投递。重放**不产生**本事件：日志里那次的决策早已
    /// 落成 `ApprovalDecided`，行按"经审批放行/拒绝"的事实闭合，不虚构等待态。
    /// `cancelled` 刻意不存在——停止只检查步间（`stop::StopFlags`），在途调用
    /// 会跑完并落 ToolResult ⇒ 日志里没有"被取消"这件事（宁缺勿假）。
    pub const TOOL_STATUS: &str = "agent://tool-status";
    /// 本轮文件改动汇总（CF §2.1，队列 Q12·C1）：投影层在轮末从写类工具结果
    /// 里的行数上报（插件侧真源）聚合产出，实时与重放同一来源。宿主**不读盘**
    /// 算 diff——读盘算出来的数不进日志，重放同一次会话会得到与当场不同的数。
    pub const FILE_CHANGES: &str = "agent://file-changes";
    /// 轮头（CF §2.1，队列 Q12·第二刀 v2.205）：本轮**收束状态**与**真正干活的
    /// 毫秒数**（墙钟区间扣掉等待用户审批的那一段）。它**不进会话日志**——不是
    /// 新事实，而是 `TurnStart` / `TurnEnd` / `ApprovalDecided`（带 `asked_at`
    /// /`decided_at`）三颗已落盘事件的汇总 ⇒ 实时与重放同一个算式、同一个数。
    /// 刻意**不带 `steps` 也不带 `ts`**：那两个读数已由 `agent://turn-done` 提供，
    /// 同一属性只许一个出处（取证批 G6 那句「先例已在场，别另发明」）。
    pub const TURN_HEADER: &str = "agent://turn-header";
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
/// `external`（㉘·B）：越界门命中——命令越出全局声明根，逐次审批、只许
/// 一次，前端据此**不渲染**「允许本次对话」。
/// `sensitive`（㉘·C）：隐私门命中——类别键（`privacy.rs` 清单，前端
/// i18n `approval.sensitive.<key>`），与 `external` 同款两键卡。
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive: Option<String>,
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
        /// step 数。v2.101 起可选：TurnEnd 的重放投影不带（日志未记录
        /// step 数，宁缺勿假）；实时路径恒有。
        #[serde(skip_serializing_if = "Option::is_none")]
        steps: Option<u32>,
        /// 收束时刻 Unix 毫秒（v2.101，SessionEvent::TurnEnd 投影 / 实时
        /// 同源）；None = 旧日志无轮界事件（前端不虚构读数）。
        #[serde(skip_serializing_if = "Option::is_none")]
        ts: Option<i64>,
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
    /// 整理段落总结（Organize Summary Design v1 §3.3）：上一轮收束的整理
    /// 产出的轮级总结在轮开场排空进日志后实时推送；重放经 project_events
    /// 同形产出（实时与重放同一变体，v2.134 uncertain 先例）。
    ContextSummary {
        session_id: String,
        text: String,
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
    /// `parts`（批 D）：附件部件（serde skip empty——纯文本消息不带字段，
    /// 前端旧消息渲染零变化）。
    UserMessage {
        session_id: String,
        content: String,
        pos: usize,
        /// 轮起点时刻（v2.101，SessionEvent::TurnStart.ts 配对投影）；
        /// None = 旧日志无轮界事件（前端不虚构读数）。
        #[serde(skip_serializing_if = "Option::is_none")]
        ts: Option<i64>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        parts: Vec<session_store::ContentPart>,
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
    /// 任务选择请求（v4.11 §3.39①）：`ask_user` 工具的卡片载荷。不进会话
    /// 日志（工具调用/结果本身在日志里，卡片只是"问人"的实时通道——与
    /// ApprovalRequest 同款纪律）。`timeout_secs` = 守夜秒数（0 = 不限）。
    AskUser {
        session_id: Option<String>,
        request_id: String,
        question: String,
        options: Vec<AskOptionPayload>,
        multi_select: bool,
        timeout_secs: u64,
    },
    /// 选择收束（v4.11 §3.39①）：应答/跳过/超时/停止都发——前端据此摘卡。
    AskResolved {
        session_id: Option<String>,
        request_id: String,
    },
    /// 传输重试（LLM.md v3.0）：兼容客户端重试前通知——前端清空本步未
    /// 完成的流式缓冲（重试轮 delta 从零重建同一气泡）并显示状态行。
    /// 只在已推过 delta 的重试发生（纯等待型重试对用户不可见）。
    LlmRetry {
        session_id: String,
        /// 第几次重试（从 1 计）。
        attempt: u32,
        /// 本次等待毫秒（退避/Retry-After 结果，状态行展示用）。
        delay_ms: u64,
    },
    /// 单轮步数到顶（LLM.md v3.0 同批）：runner 不再硬报错，正常 TurnDone
    /// 收束后发本事件——前端推 SystemNotice 如实告知 + 指引发「继续」。
    TurnCapped {
        session_id: String,
        steps: u32,
    },
    /// 崩溃恢复 uncertain（G2）：`ToolCall` 悬空（无 ToolResult 事件）且
    /// 未裁决时在全量投影里紧随工具行发出——前端渲染 uncertain 决策卡。
    /// `tool`/`args` 可能缺（assistant 声明了但 ToolCall 事件未及落盘），
    /// 前端按「细节缺失」如实渲染。只在重放产出，实时轮不产生。
    ToolUncertain {
        session_id: String,
        call_id: String,
        tool: String,
        args: Value,
    },
    /// 用户裁决（G2）：`ToolExecutionUncertain` 事件的投影——实时（裁决
    /// 命令回推）与重放同形。`decision` = completed / not_executed /
    /// inspected（serde 标签原样）；前端据此收束决策卡，随后同源的合成
    /// ToolResult 投影按裁决闭合工具行。
    UncertainResolved {
        session_id: String,
        call_id: String,
        tool: String,
        decision: String,
        #[serde(default)]
        note: String,
    },
    /// 工具行状态前置补投（见 `channels::TOOL_STATUS` 的实时/重放边界）：
    /// 审批请求发出时对那条在途调用补投 `pending_approval`，让行上的
    /// 「等待审批」不必等到结果才出现。不进日志 ⇒ 重放不产生本事件。
    ToolStatus {
        session_id: String,
        call_id: String,
        status: String,
    },
    /// 本轮文件改动（`channels::FILE_CHANGES`）：投影层在 `TurnEnd` 处从本轮
    /// 写类工具结果聚合而成——实时增量与历史重放**同一个投影函数**产出，
    /// 所以"这次改了哪些文件、各 ±几行"重启后还在。
    /// `additions`/`deletions` = `None` 是如实读数（该工具给不出真 diff，例如
    /// fs_write 覆盖已有文件），不是零；UI 对 `None` 不渲染数字（§2.1 订正块）。
    FileChanges {
        session_id: String,
        /// `SessionEvent::TurnStart.turn_id` 原样带出（u64，不是新造的字符串标识）。
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<u64>,
        files: Vec<FileChangePayload>,
    },
    /// 本轮收束的轮头（见 `channels::TURN_HEADER`）：状态四态 + 扣过等待的工作时长。
    /// 由 `runner/turn_summary.rs` 在 `TurnEnd` 处产出，实时（`finish_turn`）与
    /// 重放（`project_events`）**同一个算式**；排在 `FileChanges` 之前、`TurnDone`
    /// 之前——前端把轮头读数挂在轮尾，TurnDone 一到就冻结。
    TurnHeader {
        session_id: String,
        /// `SessionEvent::TurnEnd.turn_id` 原样带出（与 `FileChanges` 同一来源）。
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<u64>,
        state: TurnState,
        /// 本轮真正干活的毫秒数 = 墙钟区间 − Σ审批等待。
        /// `None` = 数据层读不出（旧日志无轮界时刻，或本轮的审批没有两刻）
        /// ⇒ 前端回落到墙钟读数，不拿 0 冒充"没等待"。
        #[serde(skip_serializing_if = "Option::is_none")]
        active_ms: Option<i64>,
    },
}

/// 轮收束状态（宿主侧三态）。
///
/// ⚠ 设计稿 CF §2.1 写的是**四**态（含 `running`），这里刻意只做三态：`running`
/// 的权威源是容器的本地运行态（前端 `running` 布尔与 `TurnProcess` 的 phase 机
/// 已经各有一份），而 `turn_header` 是**收束时**才发的结论——把它再带一份 running
/// 就是给同一件事造第三个出处（同类只许一种形态，§2.1 订正块记此改动）。
///
/// 三态一一对应 `SessionEvent::TurnEnd.outcome`，映射写成 `From` 而非字符串比较：
/// `TurnOutcome` 增加第三颗 `Interrupted`（v2.205）时编译器会点名这里，不会静默走
/// default 支——那正是取证批抓到的「`interrupted` 与完成同形」的产地。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Completed,
    Interrupted,
    Failed,
}

impl From<session_store::TurnOutcome> for TurnState {
    fn from(outcome: session_store::TurnOutcome) -> Self {
        use session_store::TurnOutcome as O;
        match outcome {
            O::Completed => Self::Completed,
            O::Failed => Self::Failed,
            O::Interrupted => Self::Interrupted,
        }
    }
}

/// 文件改动的一条（±N 的权威源 = 插件写类工具结果里的 `additions`/`deletions`，
/// 见 `plugins-src/tool-edit` 三个写工具与 `tool-fs` 的 fs_write 新建支）。
/// `kind` 由宿主投影单点定形（`file_change_kind`），**不由插件自报**——四个写
/// 工具各报一套 kind 名词，前端就得各有分支（同类只许一种形态）。
#[derive(Debug, Clone, Serialize)]
pub struct FileChangePayload {
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additions: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletions: Option<i64>,
}

/// `ask_user` 的选项载荷（§3.39①）：label 必填，description 可选。
#[derive(Debug, Clone, Serialize)]
pub struct AskOptionPayload {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
            AgentEvent::ContextSummary { .. } => channels::CONTEXT_SUMMARY,
            AgentEvent::ScheduleFired { .. } => channels::SCHEDULE_FIRED,
            AgentEvent::UserMessage { .. } => channels::USER_MESSAGE,
            AgentEvent::ApprovalResolved { .. } => channels::APPROVAL_RESOLVED,
            AgentEvent::Thinking { .. } => channels::THINKING,
            AgentEvent::LlmUsage { .. } => channels::LLM_USAGE,
            AgentEvent::AskUser { .. } => channels::ASK_USER,
            AgentEvent::AskResolved { .. } => channels::ASK_RESOLVED,
            AgentEvent::LlmRetry { .. } => channels::LLM_RETRY,
            AgentEvent::TurnCapped { .. } => channels::TURN_CAPPED,
            AgentEvent::ToolUncertain { .. } => channels::TOOL_UNCERTAIN,
            AgentEvent::UncertainResolved { .. } => channels::UNCERTAIN_RESOLVED,
            AgentEvent::ToolStatus { .. } => channels::TOOL_STATUS,
            AgentEvent::FileChanges { .. } => channels::FILE_CHANGES,
            AgentEvent::TurnHeader { .. } => channels::TURN_HEADER,
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
/// v4.11：持有停止标记注册表——`should_stop` 按会话查询，流式循环的
/// delta 回调点据此截断（停止生成 §3.39④）。
pub struct StreamChunkSink(AppHandle, Arc<crate::stop::StopFlags>);

impl StreamChunkSink {
    pub fn new(app: AppHandle, stop_flags: Arc<crate::stop::StopFlags>) -> Self {
        Self(app, stop_flags)
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

    /// 停止查询（v4.11 §3.39④）：按会话查 `StopFlags`——流式循环的
    /// delta 回调点返回 true 即截断（已收到的部分正文原样收束）。
    fn should_stop(&self, session_id: Option<&str>) -> bool {
        session_id.is_some_and(|sid| self.1.is_stopped(sid))
    }
}
