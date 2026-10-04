//! 捆（MemoryBundle，树 v2 §3/§17）：L0 标题 / L1 摘要 / L2 事件引用，
//! 原文一个字不存。`pending` / `source` 是实现追加字段（serde default，
//! 旧投影文件兼容）——`pending` 标占位捆（§5.3 异步整理期），`source`
//! 标生成方式（recorder / rule / placeholder，质量抽查与 A/B 用）。

use serde::{Deserialize, Serialize};

/// 事件位（SessionEvent 在会话日志中的下标）。流 append-only，位恒定。
pub type EventId = usize;

/// 树节点标识（`b1`、`b2` …顺序分配；root 见 `tree::ROOT_ID`）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BundleId(pub String);

/// 一捆：标签 + 摘要 + 指向原文的事件区间。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryBundle {
    pub bundle_id: BundleId,
    pub parent: Option<BundleId>,
    pub children: Vec<BundleId>,
    /// L0（~10 token；记录员生成或规则切分取首条用户消息前 20 字）。
    pub title: String,
    pub title_edited_by_user: bool,
    /// L1（~200 token；用户编辑后 `summary_edited_by_user` 置真，自动层
    /// 永不覆盖——纪律 5）。
    pub summary: String,
    pub summary_edited_by_user: bool,
    /// L2：原文区间（闭区间，含首尾事件位）。
    pub event_range: (EventId, EventId),
    pub created_at: i64,
    pub keywords: Vec<String>,
    pub token_count: u32,
    /// 占位捆（任务在途，§5.3）：记录员产出后整捆替换。
    #[serde(default)]
    pub pending: bool,
    /// recorder / rule / placeholder。
    #[serde(default)]
    pub source: String,
}

/// 记录员（或规则切分）的一条产出：系统校验后才落成捆。
#[derive(Debug, Clone, PartialEq)]
pub struct BundleDraft {
    pub title: String,
    pub summary: String,
    pub event_range: (EventId, EventId),
}

/// 标题硬上限：提示词要 10–20 字，校验层放宽到 40 防截断误杀。
pub const TITLE_MAX_CHARS: usize = 40;
/// 摘要硬上限（200 字是给记录员的软约束，这里是防垃圾灌爆的硬钳）。
pub const SUMMARY_MAX_CHARS: usize = 2000;

impl MemoryBundle {
    /// 从草稿落捆（调用方已校验区间合法性）。
    pub fn from_draft(id: BundleId, draft: &BundleDraft, source: &str, now_ms: i64) -> Self {
        Self {
            bundle_id: id,
            parent: None,
            children: Vec::new(),
            title: draft.title.clone(),
            title_edited_by_user: false,
            summary: draft.summary.clone(),
            summary_edited_by_user: false,
            event_range: draft.event_range,
            created_at: now_ms,
            keywords: Vec::new(),
            token_count: 0,
            pending: false,
            source: source.to_string(),
        }
    }
}

/// 草稿净化：trim → 空拒绝 → 标题/摘要硬钳（§16 常见坑「标题太长」的
/// 系统侧兜底）。失败 = 该条不可用（调用方整批转规则切分）。
pub fn sanitize_draft(
    title: &str,
    summary: &str,
    range: (EventId, EventId),
) -> Option<BundleDraft> {
    let title = title.trim();
    let summary = summary.trim();
    if title.is_empty() || summary.is_empty() || range.0 > range.1 {
        return None;
    }
    Some(BundleDraft {
        title: title.chars().take(TITLE_MAX_CHARS).collect(),
        summary: summary.chars().take(SUMMARY_MAX_CHARS).collect(),
        event_range: range,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_empty_and_inverted_range() {
        assert!(sanitize_draft("", "s", (1, 2)).is_none());
        assert!(sanitize_draft("t", "  ", (1, 2)).is_none());
        assert!(sanitize_draft("t", "s", (5, 2)).is_none());
    }

    #[test]
    fn sanitize_clips_overlong_title_and_summary() {
        let d = sanitize_draft(&"标".repeat(60), &"摘".repeat(3000), (1, 2)).unwrap();
        assert_eq!(d.title.chars().count(), TITLE_MAX_CHARS);
        assert_eq!(d.summary.chars().count(), SUMMARY_MAX_CHARS);
    }

    #[test]
    fn bundle_from_draft_marks_source() {
        let d = sanitize_draft("标题", "摘要", (3, 9)).unwrap();
        let b = MemoryBundle::from_draft(BundleId("b1".into()), &d, "recorder", 42);
        assert_eq!(b.event_range, (3, 9));
        assert_eq!(b.source, "recorder");
        assert!(!b.pending);
        assert!(!b.title_edited_by_user);
    }
}
