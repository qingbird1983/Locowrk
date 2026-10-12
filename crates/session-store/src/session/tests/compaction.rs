//! ContextCompacted 折叠（自 `tests.rs` 批 F-23 拆出，用例本体逐字未改）：摘要 + 保留
//! 尾部 / 滑动窗口无摘要纯丢早期 / 撤销压缩回到完整历史 / kept 超长是空操作 /
//! 事件 serde 往返（字段带 default，旧日志向后兼容）。

use super::*;

#[test]
fn compaction_folds_into_summary_plus_kept_tail() {
    let mut s = Session::new();
    for i in 0..4 {
        s.append(user(format!("q{i}")));
    }
    let seq = s.append(compacted(2, "earlier notes"));
    assert!(seq > 0);

    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 3, "摘要 1 条 + 保留尾部 2 条");
    let crate::Message::User { content, .. } = &msgs[0] else {
        panic!("摘要必须是 User 消息");
    };
    assert!(content.contains("earlier notes"));
    // 尾部 = 最后 kept 条原文（q2、q3）
    assert_eq!(
        msgs[1..]
            .iter()
            .map(|m| match m {
                crate::Message::User { content, .. } => content.as_str(),
                _ => "",
            })
            .collect::<Vec<_>>(),
        ["q2", "q3"]
    );

    // 快照折叠（state_at → apply）与 derive 同一语义
    assert_eq!(s.state_at(s.len()).messages, msgs);
}

#[test]
fn sliding_compaction_without_summary_drops_head_only() {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(user(format!("q{i}")));
    }
    s.append(compacted(1, ""));
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 1, "无摘要：纯丢早期，只留尾部");
    assert_eq!(msgs[0], crate::Message::user("q2"));
}

#[test]
fn compaction_rewind_restores_full_history() {
    let mut s = Session::new();
    for i in 0..4 {
        s.append(user(format!("q{i}")));
    }
    let seq = s.append(compacted(2, "earlier notes"));
    s.append(user("after compact"));
    assert_eq!(s.derive_messages().len(), 4);

    // 撤销 = rewind_to 压缩事件（前缀游标，之后的轮次一并回退）
    s.rewind_to(seq);
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 4, "回到压缩前：四条原文全部可见");
    // fork 固化回退点后可继续追加（append 恢复推进游标）
    let mut forked = s.fork(seq);
    forked.append(user("next"));
    assert_eq!(forked.cursor(), forked.len());
    assert_eq!(forked.derive_messages().len(), 5);
}

#[test]
fn compaction_kept_over_length_is_noop() {
    let mut s = Session::new();
    s.append(user("q0"));
    s.append(compacted(10, "note"));
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 1, "kept 超过长度 = 空操作（不凭空造摘要）");
    assert_eq!(msgs[0], crate::Message::user("q0"));
}

#[test]
fn compaction_event_serializes_roundtrip() {
    // serde 往返：字段带 default，旧日志无此变体不受影响（向后兼容）
    let e = compacted(2, "note");
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["type"], "context_compacted");
    assert_eq!(v["kept_messages"], 2);
    let back: SessionEvent = serde_json::from_value(v).unwrap();
    assert_eq!(back, e);
}
