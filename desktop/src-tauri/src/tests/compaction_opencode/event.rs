//! specs/13 刀⑤ · AC-18：压成事件带**摘要全文**。
//!
//! 为什么：界面上的压缩线展开体按**模型正文**那样渲染成 md（设计标准 §二⑲）
//! ⇒ 200 字预览渲出来是**断掉的一半**（`## Objective` 后面没了，剩下的段落也
//! 没了）⇒ 重放（重开应用）时那根线展开出来是半截，与实时那一趟对不上。
//!
//! 判据 = 投影那条 `ContextCompacted` 会话事件时，事件里的 `summary` 与日志里
//! 那一份**逐字相等**（不是 200 字截断）。这一个判据把「实时」与「重放」钉在
//! 同一份文本上 —— 两边不同源就一定有一边是断的。

use session_store::{Session, SessionEvent};

use crate::events::AgentEvent;
use crate::runner::projection::project_events;

/// 一段**超过 200 字**的摘要（截断会露馅 ⇒ 判据不会因为样例太短而假绿）。
fn long_summary() -> String {
    format!("## Objective\n- {}\n", "压掉的一大段。".repeat(40))
}

#[test]
fn the_compaction_event_carries_the_whole_summary() {
    let summary = long_summary();
    let mut s = Session::new();
    s.append(SessionEvent::ContextCompacted {
        kept_messages: 2,
        keep_from: 7,
        summary: summary.clone(),
        from_tokens: 9_500,
        to_tokens: 400,
        strategy: "compact".into(),
        ts: 1_700_000_000_000,
    });

    let events = project_events(s.events(), "s1");
    let compacted = events
        .iter()
        .find(|e| matches!(e, AgentEvent::ContextCompacted { .. }))
        .expect("应有一条压成事件");
    let AgentEvent::ContextCompacted {
        summary: got,
        from_tokens,
        to_tokens,
        ..
    } = compacted
    else {
        unreachable!("上面刚筛过 ContextCompacted")
    };

    assert_eq!(
        got, &summary,
        "事件带的必须是**全文**（200 字截断的 md 是断掉的一半标题）"
    );
    assert!(
        got.chars().count() > 200,
        "样例本身超过 200 字（太短的样例会让截断悄悄通过）"
    );
    // 省了多少照旧由事件带（那两格没被这次改动碰到）。
    assert_eq!((*from_tokens, *to_tokens), (9_500, 400));
}
