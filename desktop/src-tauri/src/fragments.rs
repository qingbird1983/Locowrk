//! 会话态碎片（提示词分层设计 §4.4）：桌面宿主的 pending 表 + 三碎片
//! 文本。事件处置位（整理收束 / 权限切 Plan / 轮次到顶）→ 下一次轮开场
//! 请求由内核 [`kernel_agent::FragmentInject`] 取件拼接（`<system-reminder>`
//! 包裹最后一条 user 消息）→ 取走即清（出现一次）。
//!
//! 碎片只该被看见一两次（设计稿 §3.2 注入位置判据第三行）：进 system
//! 会变成常驻议程，进会话日志会污染转录与重放——pending 表在内存，进程
//! 崩溃即失（碎片本就是「下一轮知道一下」的 ephemeral 通知，丢了无害）。
//!
//! 本模块不触 emit / 不进会话日志（影子纪律同 organize）：拼接发生在
//! 请求组装层（waterfall Replace），会话日志与转录零痕迹。

use std::collections::HashMap;
use std::sync::Mutex;

use kernel_agent::FragmentSource;

/// 整理完成（organize 三路收束同源置位）：告知历史去哪了、怎么找回。
pub const ORGANIZED: &str = "Earlier turns were just organized into the session \
     directory (the [memory tree] section in the system prompt). Their full text \
     has left your message list; call memory_expand with the chapter id when you \
     need the original content.";

/// Plan 模式（权限档切入置位、切出移除）：只读围栏不再靠模型猜。
pub const PLAN_MODE: &str = "Plan mode is active: read-only. Non-read-only tools \
     are hidden and will fail if called. Research and propose; do not modify \
     anything.";

/// 轮次到顶（run_turn 步数封顶置位）：已完成成果保留 + 「继续」出路。
pub const TURN_CAPPED: &str = "You reached the step limit for this turn; finished \
     work is preserved. The user can say 继续 to let you continue.";

/// 会话 → 待投碎片表（去重置位；取走即清）。
#[derive(Default)]
pub struct FragmentPending {
    map: Mutex<HashMap<String, Vec<String>>>,
}

impl FragmentPending {
    /// 置位（同碎片已在 = 幂等——事件可重复触发不叠重复段）。
    pub fn set(&self, session_id: &str, text: &str) {
        let text = text.to_string();
        let mut map = self.map.lock().expect("fragment pending poisoned");
        let slot = map.entry(session_id.to_string()).or_default();
        if !slot.contains(&text) {
            slot.push(text);
        }
    }

    /// 移除（Plan 切出 = 不再带，即使还没投递过）。
    pub fn remove(&self, session_id: &str, text: &str) {
        if let Ok(mut map) = self.map.lock() {
            if let Some(slot) = map.get_mut(session_id) {
                slot.retain(|t| t != text);
                if slot.is_empty() {
                    map.remove(session_id);
                }
            }
        }
    }
}

impl FragmentSource for FragmentPending {
    /// 取走并清空（drain）。无身份（None）返回空表——碎片从不进
    /// headless / 无归属请求（契约见内核 [`FragmentSource`] 文档）。
    fn take(&self, session_id: Option<&str>) -> Vec<String> {
        let Some(sid) = session_id else {
            return Vec::new();
        };
        self.map
            .lock()
            .expect("fragment pending poisoned")
            .remove(sid)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
