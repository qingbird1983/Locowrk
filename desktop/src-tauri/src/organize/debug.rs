//! 开发者观测面板（UI shell §3.36③，M2 影子运行可观测）：树投影现状 +
//! 活触发配置 + 尾巴/锚点读数。只读数据源——`memory_tree_view` 命令
//! （前端打开面板拉一次，不推送不轮询，⑫ 暴露面板同款）。
//!
//! 自 organize/mod.rs 拆出守 400 行纪律；`anchor` 与编排共用（mod.rs）。

use serde::Serialize;
use session_store::Session;

use memory_tree::{build_items, AnchorSource, OrganizeRecord, Pacing};

use super::Organizer;

/// 面板载荷（camelCase 直映前端 types）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TreeDebug {
    pub exists: bool,
    pub workspace: String,
    pub session_id: String,
    pub bundled_until: usize,
    pub bundle_count: usize,
    pub tail_events: usize,
    pub tail_tokens_est: u64,
    pub anchor_tokens: u64,
    pub anchor_source: &'static str,
    pub threshold_pct: u32,
    pub window_tokens: u64,
    pub pacing: &'static str,
    pub chunk_tokens: u64,
    pub paused_auto: bool,
    pub rebound_count: u32,
    pub pending: Option<PendingDebug>,
    pub history: Vec<OrganizeRecord>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PendingDebug {
    pub from: usize,
    pub to: usize,
    pub attempts: u32,
}

impl Organizer {
    /// 树投影 + 触发配置 + 尾巴/锚点现状（持有会话锁的同步上下文调用）。
    pub(super) fn assemble(&self, session: &Session, session_id: &str) -> TreeDebug {
        let ctx = self.settings.context();
        let mem = self.settings.memory();
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let tree = self.load_tree(&workspace, session_id);
        let items = build_items(session.events(), tree.bundled_until);
        let (anchor_tokens, anchor_source) = Self::anchor(session);
        let pacing = Pacing::parse(&mem.organize_pacing);
        TreeDebug {
            exists: self.tree_store.load(&workspace, session_id).is_some(),
            workspace,
            session_id: session_id.to_string(),
            bundled_until: tree.bundled_until,
            bundle_count: tree.bundle_count(),
            tail_events: items.len(),
            tail_tokens_est: items.iter().map(|i| i.tokens).sum(),
            anchor_tokens,
            anchor_source: match anchor_source {
                AnchorSource::LlmUsage => "llm_usage",
                AnchorSource::Estimate => "estimate",
            },
            threshold_pct: ctx.threshold_pct,
            window_tokens: self.settings.effective_window().unwrap_or(0),
            pacing: pacing.as_str(),
            chunk_tokens: mem.organize_chunk_tokens,
            paused_auto: tree.paused_auto,
            rebound_count: tree.rebound.count,
            pending: tree.pending.map(|p| PendingDebug {
                from: p.from,
                to: p.to,
                attempts: p.attempts,
            }),
            history: tree.history.clone(),
        }
    }
}
