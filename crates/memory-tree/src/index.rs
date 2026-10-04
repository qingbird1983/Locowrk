//! 反向索引（树 v2 §9.2）：事件位 → 所属捆。 locate 用 range(..=e)
//! 取前驱——事件位落在捆区间内即命中，落在捆之间（过程事件）归前一捆。

use std::collections::BTreeMap;

use crate::bundle::{BundleId, EventId};

#[derive(Debug, Clone, Default)]
pub struct EventToBundleIndex {
    map: BTreeMap<EventId, BundleId>,
}

impl EventToBundleIndex {
    /// 从捆表构建（非 pending 的捆才进索引）。
    pub fn build(
        bundles: &std::collections::BTreeMap<String, crate::bundle::MemoryBundle>,
    ) -> Self {
        let mut map = BTreeMap::new();
        for b in bundles.values() {
            if b.pending {
                continue;
            }
            map.insert(b.event_range.0, b.bundle_id.clone());
        }
        Self { map }
    }

    /// 事件位 → 所属捆（含空隙归前驱；没有任何捆覆盖其前 = None）。
    pub fn locate(&self, event_id: EventId) -> Option<&BundleId> {
        self.map.range(..=event_id).next_back().map(|(_, b)| b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::MemoryBundle;

    fn b(id: &str, range: (EventId, EventId)) -> MemoryBundle {
        MemoryBundle {
            bundle_id: BundleId(id.into()),
            event_range: range,
            pending: false,
            ..MemoryBundle::from_draft(
                BundleId(id.into()),
                &crate::bundle::sanitize_draft("t", "s", range).unwrap(),
                "recorder",
                0,
            )
        }
    }

    #[test]
    fn locate_maps_events_to_bundles_with_gap_fallback() {
        let mut bundles = std::collections::BTreeMap::new();
        bundles.insert("b1".into(), b("b1", (0, 4)));
        bundles.insert("b2".into(), b("b2", (7, 9)));
        let idx = EventToBundleIndex::build(&bundles);
        assert_eq!(idx.locate(0).map(|b| b.0.as_str()), Some("b1"));
        assert_eq!(idx.locate(4).map(|b| b.0.as_str()), Some("b1"));
        // 空隙（5–6：过程事件）归前捆
        assert_eq!(idx.locate(6).map(|b| b.0.as_str()), Some("b1"));
        assert_eq!(idx.locate(7).map(|b| b.0.as_str()), Some("b2"));
        assert_eq!(idx.locate(100).map(|b| b.0.as_str()), Some("b2"));
    }

    #[test]
    fn empty_and_pending_excluded() {
        let idx = EventToBundleIndex::build(&Default::default());
        assert!(idx.locate(0).is_none());
        let mut bundles = std::collections::BTreeMap::new();
        bundles.insert("bp".into(), {
            let mut x = b("bp", (0, 2));
            x.pending = true;
            x
        });
        assert!(EventToBundleIndex::build(&bundles).locate(1).is_none());
    }
}
