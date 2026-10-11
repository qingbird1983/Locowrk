//! 告知面（§3.37③）：两条整理系统消息的载荷 + 推送窄接口 + 两个推送方法。
//! 窄接口 `OrganizeNotifier` 的存在理由 = 隔离 tauri 运行时符号：编排器本体
//! 不触 emit，测试二进制就不会被拉进窗口栈（同 state/mod.rs 组装点注释）。
//!
//! `notify_done` / `notify_paused` 原为 `Organizer` 的私有方法（定义在 organize
//! 模块 ⇒ 对 organize 及其后代可见）。拆簇后调用方在别的文件里（trigger 的
//! plan_auto、task 的 pipeline）⇒ 必须写 `pub(super)`：**可见面与拆分前逐字相同**
//! （organize 及其后代），不是放宽。

use std::sync::Arc;

use serde::Serialize;

use memory_tree::OrganizeReason;

/// `lw://organize-done` 载荷（M3 §3.37③：整理收束系统消息，三路同源）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeDoneEvent {
    pub session_id: String,
    pub users: u64,
    pub chapters: usize,
    /// recorder | rule（rule = 降级，前端走如实文案）。
    pub mode: String,
    /// ok | degraded。
    pub status: String,
    pub reason: String,
}

/// `lw://organize-paused` 载荷：仅在熔断翻转瞬间发（§3.37③）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrganizePausedEvent {
    pub session_id: String,
    pub rebounds: u32,
    pub k: u32,
    pub m: u32,
}

/// 整理事件推送窄接口（state 组装点用 `AppHandle::emit` 实现，测试传哑
/// 闭包可断言）。隔离 tauri 运行时符号——编排器本体不触 emit，测试
/// 二进制就不会被拉进窗口栈（tao，见 state/mod.rs 组装点注释）。
pub type OrganizeNotifier = Arc<dyn Fn(&'static str, serde_json::Value) + Send + Sync>;

impl super::Organizer {
    /// 整理收束系统消息（§3.37③）：三路（手动/自动/恢复）同源。
    pub(super) fn notify_done(
        &self,
        session_id: &str,
        users: u64,
        chapters: usize,
        mode: &str,
        status: &str,
        reason: OrganizeReason,
    ) {
        let ev = OrganizeDoneEvent {
            session_id: session_id.into(),
            users,
            chapters,
            mode: mode.into(),
            status: status.into(),
            reason: reason.as_str().into(),
        };
        match serde_json::to_value(&ev) {
            Ok(v) => (self.notifier)("lw://organize-done", v),
            Err(e) => tracing::warn!("organize-done 序列化失败: {e}"),
        }
        // 尾部碎片（提示词分层设计 §4.4）：下一轮开场告知模型「历史去哪
        // 了、怎么找回」——整理对模型不再静默（此前消息列突然变短模型
        // 无感知）。fire-and-forget：置位失败不影响整理收束。
        if let Some(f) = &self.fragments {
            f.set(session_id, crate::fragments::ORGANIZED);
        }
    }

    /// 熔断翻转告知（§3.37③ 如实）。
    pub(super) fn notify_paused(&self, session_id: &str, rebounds: u32, k: u32, m: u32) {
        let ev = OrganizePausedEvent {
            session_id: session_id.into(),
            rebounds,
            k,
            m,
        };
        match serde_json::to_value(&ev) {
            Ok(v) => (self.notifier)("lw://organize-paused", v),
            Err(e) => tracing::warn!("organize-paused 序列化失败: {e}"),
        }
    }
}
