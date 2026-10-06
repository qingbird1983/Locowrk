//! 反向索引（树 v2 §9.2）：事件位 → 所属章节。 locate 用 range(..=e)
//! 取前驱——事件位落在章节区间内即命中，落在章节之间（过程事件）归前一章节。

use std::collections::BTreeMap;

use crate::chapter::{ChapterId, EventId};

#[derive(Debug, Clone, Default)]
pub struct EventToChapterIndex {
    map: BTreeMap<EventId, ChapterId>,
}

impl EventToChapterIndex {
    /// 从章节表构建（非 pending 的章节才进索引）。
    pub fn build(
        chapters: &std::collections::BTreeMap<String, crate::chapter::MemoryChapter>,
    ) -> Self {
        let mut map = BTreeMap::new();
        for b in chapters.values() {
            if b.pending {
                continue;
            }
            map.insert(b.event_range.0, b.chapter_id.clone());
        }
        Self { map }
    }

    /// 事件位 → 所属章节（含空隙归前驱；没有任何章节覆盖其前 = None）。
    pub fn locate(&self, event_id: EventId) -> Option<&ChapterId> {
        self.map.range(..=event_id).next_back().map(|(_, b)| b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chapter::MemoryChapter;

    fn b(id: &str, range: (EventId, EventId)) -> MemoryChapter {
        MemoryChapter {
            chapter_id: ChapterId(id.into()),
            event_range: range,
            pending: false,
            ..MemoryChapter::from_draft(
                ChapterId(id.into()),
                &crate::chapter::sanitize_draft("t", "s", range).unwrap(),
                "recorder",
                0,
            )
        }
    }

    #[test]
    fn locate_maps_events_to_bundles_with_gap_fallback() {
        let mut chapters = std::collections::BTreeMap::new();
        chapters.insert("b1".into(), b("b1", (0, 4)));
        chapters.insert("b2".into(), b("b2", (7, 9)));
        let idx = EventToChapterIndex::build(&chapters);
        assert_eq!(idx.locate(0).map(|b| b.0.as_str()), Some("b1"));
        assert_eq!(idx.locate(4).map(|b| b.0.as_str()), Some("b1"));
        // 空隙（5–6：过程事件）归前章节
        assert_eq!(idx.locate(6).map(|b| b.0.as_str()), Some("b1"));
        assert_eq!(idx.locate(7).map(|b| b.0.as_str()), Some("b2"));
        assert_eq!(idx.locate(100).map(|b| b.0.as_str()), Some("b2"));
    }

    #[test]
    fn empty_and_pending_excluded() {
        let idx = EventToChapterIndex::build(&Default::default());
        assert!(idx.locate(0).is_none());
        let mut chapters = std::collections::BTreeMap::new();
        chapters.insert("bp".into(), {
            let mut x = b("bp", (0, 2));
            x.pending = true;
            x
        });
        assert!(EventToChapterIndex::build(&chapters).locate(1).is_none());
    }
}
