//! Waterfall 三决策点（DESIGN.md §7.2）：`agent/pre-step`（处理用户消息前）、
//! `agent/request`（发 LLM 请求前）、`agent/request-error`（请求失败后）。
//!
//! payload 是带 schema 的 JSON——与 C ABI 同款纪律：决策方与内核解耦，
//! 只共享 JSON 形状。插件经 `ctx.events.on_waterfall` 注册决策环。

use std::time::Duration;

use plugin_api::ToolSpec;
use serde::{Deserialize, Serialize};
use session_store::Message;

use crate::CompactionOutcome;

/// 决策点事件名（§3.5）。
pub mod points {
    /// 处理用户消息前：插件可拒绝/改写消息。
    pub const PRE_STEP: &str = "agent/pre-step";
    /// 发 LLM 请求前：插件可改 provider/model/参数。
    pub const REQUEST: &str = "agent/request";
    /// 请求失败后：插件可决定 Retry。
    pub const REQUEST_ERROR: &str = "agent/request-error";
}

/// `agent/pre-step` 的 payload。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreStepPayload {
    pub messages: Vec<Message>,
}

/// `agent/request` 的 payload。`model` / `params` 可被决策环改写——
/// 审批插件通过该点注入策略参数（§14 阶段四验收）。`tools` 是工具目录
/// 快照（LLM.md：适配层负责映射各家格式），`serde(default)` 保持与
/// 旧决策方互通。`compacted` 由 ctx-compact 写回（⑨）：kernel 据此落
/// `ContextCompacted` 事件进会话日志。`system` 是请求级系统提示（⑪）：
/// Memory-Inject 把持久记忆写在这里——它在消息列表之外，ctx-compact
/// 只改写 `messages`，"系统提示永不压缩"（Context 文档 §5.3）由结构保证，
/// 会话日志也不落注入内容。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestPayload {
    pub messages: Vec<Message>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compacted: Option<CompactionOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

/// `agent/request-error` 的 payload。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestErrorPayload {
    pub error: String,
}

/// `agent/request-error` 返回 Retry 决策时的重试预算（§7.2）：同一 step 内
/// 累计次数与累计延迟上限，超限转为失败，防止决策点形成无限循环。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryBudget {
    pub max_retries: u32,
    pub max_total_delay: Duration,
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self {
            max_retries: 3,
            max_total_delay: Duration::from_secs(10),
        }
    }
}
