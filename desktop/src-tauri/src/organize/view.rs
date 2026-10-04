//! 记忆页只读视图（M3，UI shell §3.37①④）：树投影清单 / 捆清单 /
//! 捆原文切片。与开发者观测面板（debug.rs）是两个消费方——面板看
//! 「触发与管道现状」，本模块看「整理产物」；数据全部直读投影文件，
//! 不要求会话在内存、不占会话锁。只读批零写操作（编辑/删除属 M5）。

use serde::Serialize;

use memory_tree::{build_items, MaterialKind, TreeSummary};

use super::Organizer;

/// 会话切换器行（TreeSummary 直映，camelCase）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TreeSummaryView {
    pub workspace: String,
    pub session_id: String,
    pub bundle_count: usize,
    pub bundled_until: usize,
    pub pending: bool,
    pub paused_auto: bool,
    pub updated_ms: i64,
}

/// 捆清单视图（选中会话的树投影直读）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TreeBundlesView {
    pub exists: bool,
    pub workspace: String,
    pub session_id: String,
    pub bundled_until: usize,
    pub paused_auto: bool,
    pub bundles: Vec<BundleView>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BundleView {
    pub bundle_id: String,
    pub title: String,
    pub summary: String,
    /// recorder / rule / placeholder（来源如实，M3 只读不隐藏降级产物）。
    pub source: String,
    pub pending: bool,
    pub created_at: i64,
    pub event_from: usize,
    pub event_to: usize,
}

/// 原文切片行：与记录员材料同形同源（build_items）——看原文 = 看记录员
/// 看到的材料；kind 走 MaterialKind 的 serde 形（user/assistant/…）。
#[derive(Serialize, Clone)]
pub struct OriginalItem {
    /// 会话日志事件位（与捆 event_range 同一坐标系）。
    pub index: usize,
    pub kind: MaterialKind,
    pub text: String,
}

impl Organizer {
    /// 树投影清单（会话切换器数据源，最近更新降序）。
    pub fn tree_sessions(&self) -> Vec<TreeSummaryView> {
        self.tree_store
            .scan_trees()
            .into_iter()
            .map(
                |TreeSummary {
                     workspace,
                     session_id,
                     bundle_count,
                     bundled_until,
                     pending,
                     paused_auto,
                     updated_ms,
                 }| TreeSummaryView {
                    workspace,
                    session_id,
                    bundle_count,
                    bundled_until,
                    pending,
                    paused_auto,
                    updated_ms,
                },
            )
            .collect()
    }

    /// 选中会话的捆清单（投影缺/坏 = exists=false，前端走空态）。
    pub fn tree_bundles(&self, session_id: &str, workspace: &str) -> TreeBundlesView {
        let Some(tree) = self.tree_store.load(workspace, session_id) else {
            return TreeBundlesView {
                exists: false,
                workspace: workspace.to_string(),
                session_id: session_id.to_string(),
                bundled_until: 0,
                paused_auto: false,
                bundles: Vec::new(),
            };
        };
        // BTreeMap 是字典序（b10 < b2），列表按捆序号数值排（创建序）。
        let mut sorted: Vec<&memory_tree::MemoryBundle> = tree.bundles.values().collect();
        sorted.sort_by_key(|b| {
            b.bundle_id
                .0
                .strip_prefix('b')
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        });
        let bundles = sorted
            .into_iter()
            .map(|b| BundleView {
                bundle_id: b.bundle_id.0.clone(),
                title: b.title.clone(),
                summary: b.summary.clone(),
                source: b.source.clone(),
                pending: b.pending,
                created_at: b.created_at,
                event_from: b.event_range.0,
                event_to: b.event_range.1,
            })
            .collect();
        TreeBundlesView {
            exists: true,
            workspace: tree.workspace,
            session_id: tree.session_id,
            bundled_until: tree.bundled_until,
            paused_auto: tree.paused_auto,
            bundles,
        }
    }

    /// 捆原文切片（磁盘会话日志重读；会话已删 / 捆不存在 = Err 文案直显）。
    pub fn tree_original(
        &self,
        session_id: &str,
        workspace: &str,
        bundle_id: &str,
    ) -> Result<Vec<OriginalItem>, String> {
        let tree = self
            .tree_store
            .load(workspace, session_id)
            .ok_or_else(|| "树投影不存在或已损坏".to_string())?;
        let bundle = tree
            .bundles
            .values()
            .find(|b| b.bundle_id.0 == bundle_id)
            .ok_or_else(|| format!("捆不存在：{bundle_id}"))?;
        let path = session_store::session_file(&self.sessions_dir, session_id)
            .map_err(|e| format!("会话日志定位失败: {e}"))?;
        let (session, _) =
            session_store::load_session(&path).map_err(|e| format!("会话日志读取失败: {e}"))?;
        let (from, to) = bundle.event_range;
        let events = session.events();
        if from >= events.len() {
            return Err("捆指向的事件区间超出会话日志（会话可能已截断重写）".into());
        }
        let end = (to + 1).min(events.len());
        Ok(build_items(&events[from..end], from)
            .into_iter()
            .map(|i| OriginalItem {
                index: i.event,
                kind: i.kind,
                text: i.text,
            })
            .collect())
    }
}
