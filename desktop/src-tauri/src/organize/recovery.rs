//! 崩溃恢复（§5.3，自 organize/mod.rs 拆出守 400 行纪律）：启动扫描
//! 投影里的在途整理任务并重投——任务与占位章节随会话持久化（§5.3 契约），
//! 材料从磁盘会话日志重读；会话已删 = 任务终局放弃（占位撤除，尾巴
//! 留在原文流里，原文无损）。

use std::sync::Arc;

use memory_tree::{build_items, MemoryManagerUsage, OrganizeReason};

use super::{Organizer, TaskPayload};

impl Organizer {
    /// 启动重投入口（state/init_state 调用）：逐个在途任务决策后 spawn。
    pub fn recover_pending(self: &Arc<Self>) {
        for hit in self.tree_store.scan_pending() {
            if let Some(task) = self.plan_recovery(&hit.session_id) {
                self.spawn(task);
            }
        }
    }

    /// 单个在途任务的恢复决策（同步，测试可直呼；产物经 spawn 跑管道）。
    pub(crate) fn plan_recovery(&self, session_id: &str) -> Option<TaskPayload> {
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        let pending = tree.pending.clone()?;
        let path = match session_store::session_file(&self.sessions_dir, session_id) {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!(session = %session_id, "在途整理任务的会话定位失败：放弃任务");
                tree.drop_pending(Self::record(
                    OrganizeReason::Recovery,
                    "recorder",
                    "aborted",
                    pending.from,
                    pending.to,
                    0,
                    MemoryManagerUsage::default(),
                    0,
                ));
                self.tree_store.save(&tree).ok();
                return None;
            }
        };
        let Ok((session, _)) = session_store::load_session(&path) else {
            tracing::warn!(session = %session_id, "在途整理任务的会话日志已删：放弃任务");
            tree.drop_pending(Self::record(
                OrganizeReason::Recovery,
                "recorder",
                "aborted",
                pending.from,
                pending.to,
                0,
                MemoryManagerUsage::default(),
                0,
            ));
            self.tree_store.save(&tree).ok();
            return None;
        };
        let items = build_items(&session.events()[pending.from..=pending.to], pending.from);
        if items.is_empty() {
            return None;
        }
        let users_total = memory_tree::user_count(session.events());
        tracing::info!(session = %session_id, "重投在途整理任务");
        Some(TaskPayload {
            session_id: session_id.to_string(),
            workspace,
            items,
            users_total,
            reason: OrganizeReason::Recovery,
        })
    }
}
