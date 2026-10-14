//! 事件 serde 兼容（⑩③，exposure 文档 §3.4）：旧日志没有 plugin_id 字段，
//! 解析按 None 兜底；新日志带值原样往返。

use serde_json::json;

use crate::event::{SessionEvent, TurnId, TurnOutcome};
use plugin_api::{CallId, SideEffect};

#[test]
fn tool_call_without_plugin_id_parses_as_none() {
    let old = json!({
        "type": "tool_call",
        "call_id": "call-1",
        "tool": "echo",
        "args": { "text": "hi" },
        "plugin_version": "1.2.0",
        "side_effect": { "kind": "pure" }
    });
    let e: SessionEvent = serde_json::from_value(old).unwrap();
    match e {
        SessionEvent::ToolCall { plugin_id, .. } => assert_eq!(plugin_id, None),
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn tool_call_plugin_id_roundtrips() {
    let e = SessionEvent::ToolCall {
        call_id: CallId("c1".into()),
        tool: "fs_read".into(),
        args: json!({ "path": "/tmp/a" }),
        plugin_id: Some("tool-fs".into()),
        plugin_version: semver::Version::new(1, 2, 0),
        side_effect: SideEffect::Pure,
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["plugin_id"], json!("tool-fs"));
    let back: SessionEvent = serde_json::from_value(v).unwrap();
    assert_eq!(back, e);
}

/// 审批两时刻的 serde 面（队列 Q12·第二刀 v2.205，决策点一甲案）。
/// 三件事各自钉住：①**旧日志整行照读**（缺字段 = None，不是报错也不是丢行——
/// `load_session` 对坏行只 `skipped += 1`，把它判成坏行等于历史会话少一条）；
/// ②带值原样往返；③两刻缺省时**不落键** ⇒ 新写的 jsonl 在无等待路径上与改前
/// 字节同形，这就是"零迁移"的实际含义（不是口头承诺）。
#[test]
fn approval_timestamps_are_optional_and_skipped_when_absent() {
    let old = json!({
        "type": "approval_decided",
        "call_id": "call-9",
        "allowed": true,
        "reason": null
    });
    let e: SessionEvent = serde_json::from_value(old).unwrap();
    match &e {
        SessionEvent::ApprovalDecided {
            asked_at,
            decided_at,
            ..
        } => {
            assert_eq!(*asked_at, None, "旧日志缺字段 = 不知道等了多久");
            assert_eq!(*decided_at, None);
        }
        other => panic!("wrong variant: {other:?}"),
    }
    // 回写不落键：旧消费者读的字节形状不变
    let back = serde_json::to_value(&e).unwrap();
    assert!(
        back.get("asked_at").is_none() && back.get("decided_at").is_none(),
        "None 必须被 skip_serializing 掉：{back}"
    );

    let new = SessionEvent::ApprovalDecided {
        call_id: CallId("call-9".into()),
        allowed: true,
        reason: None,
        asked_at: Some(1_700_000_000_000),
        decided_at: Some(1_700_000_002_500),
    };
    let v = serde_json::to_value(&new).unwrap();
    assert_eq!(v["asked_at"], json!(1_700_000_000_000_i64));
    assert_eq!(v["decided_at"], json!(1_700_000_002_500_i64));
    assert_eq!(serde_json::from_value::<SessionEvent>(v).unwrap(), new);
}

/// `TurnOutcome::Interrupted`（同刀新增的第三态）：新值写读同形，而旧日志里的
/// `"completed"` **不会被 reinterpret** 成 interrupted——那一轮究竟是不是被停的，
/// 日志没记着就不猜（与 `TurnDone.steps` 的「宁缺勿假」一条纪律）。
#[test]
fn interrupted_outcome_roundtrips_while_old_values_stay_completed() {
    let e = SessionEvent::TurnEnd {
        turn_id: TurnId(4),
        outcome: TurnOutcome::Interrupted,
        ts: 1_700_000_000_500,
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["outcome"], json!("interrupted"));
    assert_eq!(serde_json::from_value::<SessionEvent>(v).unwrap(), e);

    let old = json!({
        "type": "turn_end",
        "turn_id": 4,
        "outcome": "completed",
        "ts": 1_700_000_000_500_i64
    });
    assert!(matches!(
        serde_json::from_value::<SessionEvent>(old).unwrap(),
        SessionEvent::TurnEnd {
            outcome: TurnOutcome::Completed,
            ..
        }
    ));
}
