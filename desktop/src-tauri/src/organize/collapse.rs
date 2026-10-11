//! 空间急救面（M4 §8.3 自动收起，缺省关）：占用达到会话生效窗口阈值后收起最早
//! 展开的章节，一次一个逐步泄压；生效窗口走 v2.137 四层链自动识别（模型行 ?? 目录
//! ?? 档位），全未知不猜口径。先于整理判定调用——泄压不依赖触发。
//!
//! `auto_collapse` 原为私有方法，被 mod.rs 的 `after_turn` 调用 ⇒ 拆簇后 `pub(super)`
//! （可见面不变：organize 及其后代）。

use session_store::Session;

use crate::timeutil::now_ms;

impl super::Organizer {
    /// 自动收起（M4 §8.3，缺省关）：collapse 开启 + 占用 ≥ 会话生效窗口
    /// 阈值 → 收起最早展开的章节（一次一个，逐步泄压）。窗口走 v2.137
    /// 四层链自动识别（模型行 ?? 目录 ?? 档位）；全未知不猜口径。
    pub(super) fn auto_collapse(&self, session: &Session, session_id: &str) {
        let (enabled, threshold_pct) = self.settings.collapse_config();
        if !enabled {
            return;
        }
        let ov = self.model_overrides.get(session_id);
        let Some(window) = crate::settings::session_window(
            &self.settings.llm_config(),
            ov.as_ref(),
            Some(&self.catalog),
        ) else {
            return;
        };
        if window == 0 {
            return;
        }
        let (anchor, _) = Self::anchor(session);
        if anchor.saturating_mul(100) < window.saturating_mul(threshold_pct as u64) {
            return;
        }
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        let Some(id) = tree.oldest_expanded() else {
            return;
        };
        tree.mark_collapsed(&id, now_ms());
        if self.tree_store.save(&tree).is_ok() {
            self.sync_tree_md(&workspace);
            tracing::info!(
                session = %session_id, chapter = %id,
                "自动收起（占用 {anchor} / {window} ≥ {threshold_pct}%）"
            );
        }
    }
}
