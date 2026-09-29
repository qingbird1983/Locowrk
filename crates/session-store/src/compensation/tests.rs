//! `compensation` 模块单测：失败不补偿、Pure 跳过、逆序输出、幂等去重。

use semver::Version;
use serde_json::json;

use plugin_api::{CallId, SideEffect};

use super::{CompensatedLedger, Compensation, plan};
use crate::event::SessionEvent;

fn call(seq: &str, effect: SideEffect) -> SessionEvent {
    SessionEvent::ToolCall {
        call_id: CallId(seq.into()),
        tool: format!("tool-{seq}"),
        args: json!({}),
        plugin_version: Version::new(0, 1, 0),
        side_effect: effect,
    }
}

fn result(seq: &str, error: Option<String>) -> SessionEvent {
    SessionEvent::ToolResult {
        call_id: CallId(seq.into()),
        output: json!(null),
        error,
    }
}

#[test]
fn plan_skips_failed_pure_and_orders_reverse() {
    let events = vec![
        call("c1", SideEffect::Pure),
        result("c1", None),
        call(
            "c2",
            SideEffect::Compensable {
                undo_tool: "undo_2".into(),
            },
        ),
        result("c2", None),
        call("c3", SideEffect::Irreversible),
        result("c3", None),
        call(
            "c4",
            SideEffect::Compensable {
                undo_tool: "undo_4".into(),
            },
        ),
        result("c4", Some("boom".into())), // 失败：副作用未必发生，不补偿（§6.3）
    ];

    assert_eq!(
        plan(&events, 0),
        vec![
            // 逆序：c3 先于 c2
            Compensation::ManualIntervention {
                call_id: CallId("c3".into()),
                reason: "irreversible side effect".into(),
            },
            Compensation::Undo {
                call_id: CallId("c2".into()),
                undo_tool: "undo_2".into(),
            },
        ]
    );
}

#[test]
fn plan_after_seq_limits_window() {
    let events = vec![
        call(
            "c1",
            SideEffect::Compensable {
                undo_tool: "undo_1".into(),
            },
        ),
        result("c1", None),
    ];
    // 回退点在工具调用之前（seq 1 = result 之后）→ 空计划
    assert!(plan(&events, 1).is_empty());
    assert_eq!(plan(&events, 0).len(), 1);
}

#[test]
fn ledger_is_idempotent_by_call_id() {
    let mut ledger = CompensatedLedger::default();
    let id = CallId("c2".into());
    assert!(ledger.claim(&id)); // 首次补偿
    assert!(!ledger.claim(&id)); // 重试：不再执行
    assert!(ledger.contains(&id));
    assert_eq!(ledger.len(), 1);
}
