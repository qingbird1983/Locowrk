//! 触发与决策面：`plan_auto`（轮次收束后的自动判定，含 M4 模式 B 溢出整理与
//! §5.5 反弹熔断）/ `plan_manual`（`/整理`，不受熔断限制且复位熔断态）。两者都是
//! **同步**决策、可测：写路径含反弹账本与占位章节落盘，产物是管道输入 TaskPayload
//! （材料项克隆自触发瞬间的会话日志——事件流 append-only，区间恒定有效）。
//! 执行管道在 task.rs，空间急救在 collapse.rs。

use memory_tree::{
    build_items, should_auto_organize, MaterialItem, OrganizeReason, Pacing, ReboundDecision,
    TriggerInput,
};
use session_store::Session;

use crate::settings::TailStrategy;

use super::assemble::window_start_event;

/// 决策产物：管道输入（材料项克隆自触发瞬间的会话日志——事件流
/// append-only，区间恒定有效）。
pub(crate) struct TaskPayload {
    pub session_id: String,
    pub workspace: String,
    pub items: Vec<MaterialItem>,
    pub users_total: u64,
    pub reason: OrganizeReason,
}

impl super::Organizer {
    /// 自动触发决策（轮次收束后、持有会话锁的同步上下文调用）。写路径：
    /// 反弹账本落投影文件；返回 None = 不整理。
    pub(crate) fn plan_auto(&self, session: &Session, session_id: &str) -> Option<TaskPayload> {
        let mem = self.settings.memory();
        let dev = self.settings.developer();
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        // M4 模式 B 溢出整理（§3.38②）：固定窗口档，窗口滑出的
        // 孤儿轮（chaptered_until < 窗口起点）既不在尾巴也不在目录，必须及时
        // 归章节——例行维护：**先于熔断判定**（熔断暂停的是阈值/增量烧钱信号，
        // 不是固定节奏）且**不记反弹**（管道按 reason 跳过 note_organized）。
        if self.settings.tail_config().0 == TailStrategy::Window && tree.pending.is_none() {
            let start = window_start_event(session.events(), mem.tail_window_turns);
            if tree.chaptered_until < start && start > 0 {
                let events = session.events();
                let items = build_items(&events[tree.chaptered_until..start], tree.chaptered_until);
                if !items.is_empty() {
                    let users_total = memory_tree::user_count(session.events());
                    return Some(TaskPayload {
                        session_id: session_id.to_string(),
                        workspace,
                        items,
                        users_total,
                        reason: OrganizeReason::WindowOverflow,
                    });
                }
            }
        }
        if tree.paused_auto || tree.pending.is_some() {
            return None;
        }
        let items = build_items(session.events(), tree.chaptered_until);
        let tail_tokens: u64 = items.iter().map(|i| i.tokens).sum();
        let (anchor_tokens, _) = Self::anchor(session);
        let reason = should_auto_organize(&TriggerInput {
            anchor_tokens,
            threshold_tokens: self.settings.organize_threshold_tokens(),
            pacing: Pacing::parse(&mem.organize_pacing),
            chunk_tokens: mem.organize_chunk_tokens,
            tail_tokens,
            tail_events: items.len(),
        })?;
        // 反弹熔断（§5.5）：连续 m 次反弹 → 暂停本会话自动整理并跳过本次
        let users_total = memory_tree::user_count(session.events());
        if tree.on_auto_trigger(users_total, dev.organize_rebound_k, dev.organize_rebound_m)
            == ReboundDecision::Pause
        {
            tracing::warn!(session = %session_id, "整理反弹熔断：暂停本会话自动整理（/整理 不受限）");
            self.tree_store.save(&tree).ok();
            // plan_auto 在 paused_auto 已置位时早退，走到这里即翻转瞬间
            self.notify_paused(
                session_id,
                tree.rebound.count,
                dev.organize_rebound_k,
                dev.organize_rebound_m,
            );
            return None;
        }
        self.tree_store.save(&tree).ok();
        Some(TaskPayload {
            session_id: session_id.to_string(),
            workspace,
            items,
            users_total,
            reason,
        })
    }

    /// 手动 `/整理`（不受熔断限制且复位熔断态，§3.36①）。
    pub(crate) fn plan_manual(
        &self,
        session: &Session,
        session_id: &str,
    ) -> Result<TaskPayload, String> {
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        if tree.pending.is_some() {
            return Err("已有整理任务在途".into());
        }
        let items = build_items(session.events(), tree.chaptered_until);
        if items.is_empty() {
            return Err("没有可整理的尾巴".into());
        }
        let users_total = memory_tree::user_count(session.events());
        tree.mark_manual(users_total);
        self.tree_store.save(&tree).ok();
        Ok(TaskPayload {
            session_id: session_id.to_string(),
            workspace,
            items,
            users_total,
            reason: OrganizeReason::Manual,
        })
    }
}
