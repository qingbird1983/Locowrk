//! 记忆页只读视图（M3，UI shell §3.37①④）：树投影清单 / 章节清单 /
//! 章节原文切片。与开发者观测面板（debug.rs）是两个消费方——面板看
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
    pub chapter_count: usize,
    pub chaptered_until: usize,
    pub pending: bool,
    pub paused_auto: bool,
    pub updated_ms: i64,
}

/// 章节清单视图（选中会话的树投影直读）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TreeChaptersView {
    pub exists: bool,
    pub workspace: String,
    pub session_id: String,
    pub chaptered_until: usize,
    pub paused_auto: bool,
    pub chapters: Vec<ChapterView>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChapterView {
    pub chapter_id: String,
    pub title: String,
    pub summary: String,
    /// recorder / rule / placeholder（来源如实，M3 只读不隐藏降级产物）。
    pub source: String,
    pub pending: bool,
    pub created_at: i64,
    pub event_from: usize,
    pub event_to: usize,
}

/// 原文切片行：与记忆管理器材料同形同源（build_items）——看原文 = 看记忆管理器
/// 看到的材料；kind 走 MaterialKind 的 serde 形（user/assistant/…）。
#[derive(Serialize, Clone)]
pub struct OriginalItem {
    /// 会话日志事件位（与章节 event_range 同一坐标系）。
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
                     chapter_count,
                     chaptered_until,
                     pending,
                     paused_auto,
                     updated_ms,
                 }| TreeSummaryView {
                    workspace,
                    session_id,
                    chapter_count,
                    chaptered_until,
                    pending,
                    paused_auto,
                    updated_ms,
                },
            )
            .collect()
    }

    /// 选中会话的章节清单（投影缺/坏 = exists=false，前端走空态）。
    pub fn tree_chapters(&self, session_id: &str, workspace: &str) -> TreeChaptersView {
        let Some(tree) = self.tree_store.load(workspace, session_id) else {
            return TreeChaptersView {
                exists: false,
                workspace: workspace.to_string(),
                session_id: session_id.to_string(),
                chaptered_until: 0,
                paused_auto: false,
                chapters: Vec::new(),
            };
        };
        // BTreeMap 是字典序（b10 < b2），列表按章节序号数值排（创建序）。
        let mut sorted: Vec<&memory_tree::MemoryChapter> = tree.chapters.values().collect();
        sorted.sort_by_key(|b| {
            b.chapter_id
                .0
                .strip_prefix('b')
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        });
        let chapters = sorted
            .into_iter()
            .map(|b| ChapterView {
                chapter_id: b.chapter_id.0.clone(),
                title: b.title.clone(),
                summary: b.summary.clone(),
                source: b.source.clone(),
                pending: b.pending,
                created_at: b.created_at,
                event_from: b.event_range.0,
                event_to: b.event_range.1,
            })
            .collect();
        TreeChaptersView {
            exists: true,
            workspace: tree.workspace,
            session_id: tree.session_id,
            chaptered_until: tree.chaptered_until,
            paused_auto: tree.paused_auto,
            chapters,
        }
    }

    /// 章节原文切片（磁盘会话日志重读；会话已删 / 章节不存在 = Err 文案直显）。
    pub fn tree_original(
        &self,
        session_id: &str,
        workspace: &str,
        chapter_id: &str,
    ) -> Result<Vec<OriginalItem>, String> {
        let tree = self
            .tree_store
            .load(workspace, session_id)
            .ok_or_else(|| "树投影不存在或已损坏".to_string())?;
        let chapter = tree
            .chapters
            .values()
            .find(|b| b.chapter_id.0 == chapter_id)
            .ok_or_else(|| format!("章节不存在：{chapter_id}"))?;
        let path = session_store::session_file(&self.sessions_dir, session_id)
            .map_err(|e| format!("会话日志定位失败: {e}"))?;
        let (session, _) =
            session_store::load_session(&path).map_err(|e| format!("会话日志读取失败: {e}"))?;
        let (from, to) = chapter.event_range;
        let events = session.events();
        if from >= events.len() {
            return Err("章节指向的事件区间超出会话日志（会话可能已截断重写）".into());
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
