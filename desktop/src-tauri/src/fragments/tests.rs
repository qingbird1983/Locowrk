//! FragmentPending 单测：幂等置位 / 取走即清 / remove 定向移除 /
//! 无身份不取 / FragmentSource 契约。

use super::{FragmentPending, PLAN_MODE, TURN_CAPPED};

#[test]
fn set_is_idempotent_per_session() {
    let p = FragmentPending::default();
    p.set("s1", TURN_CAPPED);
    p.set("s1", TURN_CAPPED);
    p.set("s1", PLAN_MODE);
    let taken = kernel_agent::FragmentSource::take(&p, Some("s1"));
    assert_eq!(taken.len(), 2, "同碎片不叠加");
    assert_eq!(taken[0], TURN_CAPPED);
    assert_eq!(taken[1], PLAN_MODE);
}

#[test]
fn take_drains_and_sessions_are_isolated() {
    let p = FragmentPending::default();
    p.set("s1", TURN_CAPPED);
    p.set("s2", PLAN_MODE);
    assert_eq!(
        kernel_agent::FragmentSource::take(&p, Some("s1")),
        vec![TURN_CAPPED.to_string()]
    );
    // 取走即清
    assert!(kernel_agent::FragmentSource::take(&p, Some("s1")).is_empty());
    // 会话隔离：s2 的不受影响
    assert_eq!(
        kernel_agent::FragmentSource::take(&p, Some("s2")),
        vec![PLAN_MODE.to_string()]
    );
    assert!(kernel_agent::FragmentSource::take(&p, Some("s3")).is_empty());
}

/// 无身份（None）返回空表且不取走任何会话的碎片（契约：碎片从不进
/// headless / 无归属请求）。
#[test]
fn none_session_takes_nothing() {
    let p = FragmentPending::default();
    p.set("s1", TURN_CAPPED);
    assert!(kernel_agent::FragmentSource::take(&p, None).is_empty());
    assert_eq!(
        kernel_agent::FragmentSource::take(&p, Some("s1")),
        vec![TURN_CAPPED.to_string()],
        "碎片仍在原处"
    );
}

/// Plan 切出 = 定向移除该碎片（其余保留），即使还没投递过。
#[test]
fn remove_targets_one_fragment() {
    let p = FragmentPending::default();
    p.set("s1", PLAN_MODE);
    p.set("s1", TURN_CAPPED);
    p.remove("s1", PLAN_MODE);
    assert_eq!(
        kernel_agent::FragmentSource::take(&p, Some("s1")),
        vec![TURN_CAPPED.to_string()]
    );
    // 不存在的碎片 / 不存在的会话 = 安静无操作
    p.remove("s1", TURN_CAPPED);
    p.remove("s404", PLAN_MODE);
}

/// 碎片文本的形状契约：非空、无闭合标签边界（内核 wrap 再转义一道，
/// 文本本身也不带标签——双保险）。
#[test]
fn fragment_texts_are_plain() {
    for t in [PLAN_MODE, TURN_CAPPED] {
        assert!(!t.is_empty());
        assert!(!t.contains("</system-reminder>"));
        assert!(!t.contains("<system-reminder>"));
    }
}
