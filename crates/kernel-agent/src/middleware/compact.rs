//! 上下文压缩的**遗留契约面**（Context and Memory.md §3 历史沿革）。
//!
//! 原 `CtxCompact`（`agent/request` waterfall 监听器）连同 `CompactConfig` /
//! `CompactStrategy` / `suggested_threshold` / 稳定期账本 / 切点与摘要链已随
//! **审计修复批 C 删除**：桌面压缩链 v2.137 整面退役（上下文节奏改由记忆树
//! 的整理边界承担），而 `kernel-bin` 的 headless 侧其实从未注册过它——
//! 审计报告 H2 判的「存在但从未接线」正是这一条（构造点全在测试里，注释却
//! 写着「headless 侧仍注册」）。删除前 = 306 行本体 + 509 行测试，逐行留在
//! git 历史里，将来要真做 headless 压缩再照历史接回，不必现在养着死码。
//!
//! 这里只留两件**仍在服务**的东西：
//!
//! * [`CompactionOutcome`] —— `RequestPayload::compacted` 的载荷类型。kernel
//!   据此落 `SessionEvent::ContextCompacted` 进会话日志，而旧日志的重放链路
//!   是活的（session-store 折叠 → desktop `runner.rs` 投影 → 前端通知与转录
//!   → `notifications.rs` 入账），所以这个类型与 payload 字段都删不得。
//!   字段本身同样是**插件可扩展面**：waterfall 的任何 `agent/request` 决策方
//!   都能改写它（现网无生产写入点是事实，不等于契约失效）。
//! * [`estimate_tokens`] —— 全仓唯一的 token 粗估尺：用量环/阈值进度条/
//!   maxOutput 预检钳制/子代理窗口装不下的判断/树尾巴估算都吃它（v2.138
//!   「真实锚点优先、无上报才回落估算」里的回落那一半）。

use serde::{Deserialize, Serialize};
use session_store::Message;

/// 压缩结果：监听器写回 `RequestPayload::compacted`，kernel 据此落
/// `ContextCompacted` 事件。summary 存全文（日志折叠重建需要）；
/// `strategy` 是当时的策略名字符串（枚举随本体一起退役，历史事件只读不写）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactionOutcome {
    pub from_tokens: u64,
    pub to_tokens: u64,
    pub strategy: String,
    pub summary: String,
    pub kept_messages: usize,
    /// 保留区指针（specs/13 刀② · C5）：保留区在消息列里的起点。0 = 旧日志
    /// 形状（没有指针）⇒ 折叠路径按 `kept_messages` 从尾部回退。
    #[serde(default)]
    pub keep_from: usize,
}

/// 粗估 token：全消息序列化字节 / 4（转义带来的高估当安全余量；CJK 偏
/// 低估，宁早不算晚）。runner 进度条与压缩判定历史上共用同一把尺。
/// 批 D：附件 parts 平铺（图片 1024 / 文件 1000，`ContentPart::est_tokens`）
/// ——图片推高估算 → 整理更早触发 + maxOutput 预检钳制自动收窄（同尺）。
pub fn estimate_tokens(messages: &[Message]) -> u64 {
    messages.iter().map(estimate_one).sum()
}

fn estimate_one(m: &Message) -> u64 {
    let base = serde_json::to_string(m).map_or(0, |s| s.len() as u64) / 4;
    let parts: u64 = match m {
        Message::User { parts, .. } => parts.iter().map(|p| p.est_tokens()).sum(),
        _ => 0,
    };
    base + parts
}

#[cfg(test)]
mod tests;
