//! `snapshot` 模块单测：逐级 upcast、缺口与过新报错、恢复会话。

use serde_json::{json, Value};

use super::{Snapshot, Upcaster, Upcasters, SNAPSHOT_SCHEMA_VERSION};
use crate::session::Session;

/// v0 → v1：状态字段 `items` 改名 `messages`。
struct RenameItemsToMessages;

impl Upcaster for RenameItemsToMessages {
    fn source_version(&self) -> u32 {
        0
    }

    fn upcast(&self, mut state: Value) -> Value {
        if let Some(items) = state.get("items").cloned() {
            let obj = state.as_object_mut().unwrap();
            obj.remove("items");
            obj.insert("messages".into(), items);
        }
        state
    }
}

#[test]
fn snapshot_roundtrip_at_current_version() {
    let mut s = Session::new();
    s.append(crate::event::SessionEvent::UserMessage {
        content: "hi".into(),
    });
    let snap = s.snapshot_at(1);

    let restored = Session::from_snapshot(snap, &Upcasters::default()).unwrap();
    assert_eq!(restored.state_at(0).messages.len(), 1);
    // 快照只带状态：恢复后日志为空，事件流另存
    assert!(restored.is_empty());
}

#[test]
fn load_upcasts_step_by_step() {
    let mut ups = Upcasters::default();
    ups.register(Box::new(RenameItemsToMessages));

    let raw: Value = json!({
        "schema_version": 0,
        "seq": 1,
        "state": { "items": [ { "role": "user", "content": "hi" } ] }
    });
    let snap: Snapshot = serde_json::from_value(raw).unwrap();
    let upcast = ups.to_current(snap).unwrap();
    assert_eq!(upcast.schema_version, SNAPSHOT_SCHEMA_VERSION);

    let restored = Session::from_snapshot(upcast, &ups).unwrap();
    assert_eq!(restored.state_at(0).messages.len(), 1);
}

#[test]
fn missing_upcaster_is_schema_gap() {
    let snap = Snapshot {
        schema_version: 0,
        seq: 0,
        state: json!({ "items": [] }),
    };
    assert!(matches!(
        Upcasters::default().to_current(snap),
        Err(crate::error::StoreError::SchemaGap { found: 0, .. })
    ));
}

#[test]
fn newer_snapshot_is_rejected() {
    let snap = Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION + 1,
        seq: 0,
        state: json!({ "messages": [] }),
    };
    assert!(matches!(
        Upcasters::default().to_current(snap),
        Err(crate::error::StoreError::TooNew { .. })
    ));
}
