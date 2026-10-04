//! tree 单测：占位捆生命周期 / 反弹熔断 k/m / 历史封顶。

use super::*;

fn bundle(id: &str, range: (EventId, EventId)) -> MemoryBundle {
    MemoryBundle {
        bundle_id: BundleId(id.into()),
        parent: None,
        children: Vec::new(),
        title: format!("捆{id}"),
        title_edited_by_user: false,
        summary: "摘要".into(),
        summary_edited_by_user: false,
        event_range: range,
        created_at: 1,
        keywords: Vec::new(),
        token_count: 0,
        pending: false,
        source: "recorder".into(),
    }
}

fn record(status: &str) -> OrganizeRecord {
    OrganizeRecord {
        ts: 1,
        reason: "manual".into(),
        mode: "recorder".into(),
        status: status.into(),
        from: 0,
        to: 9,
        bundles: 1,
        recorder_input: 100,
        recorder_output: 50,
        duration_ms: 10,
    }
}

#[test]
fn placeholder_then_apply_replaces_and_advances() {
    let mut t = MemoryTree::new("ws", "s1");
    let task = t.insert_placeholder(10, 19, 5).expect("任务可建");
    assert_eq!(t.bundle_count(), 1);
    assert!(t.bundles[&task.placeholder_id.0].pending);

    t.apply_organized(
        vec![bundle("b1", (10, 14)), bundle("b2", (15, 19))],
        19,
        record("ok"),
    );
    assert!(t.pending.is_none());
    assert_eq!(t.bundle_count(), 2); // 占位捆已撤
    assert_eq!(t.bundled_until, 20);
    assert_eq!(t.history.len(), 1);
}

#[test]
fn second_task_rejected_while_pending() {
    let mut t = MemoryTree::new("ws", "s1");
    assert!(t.insert_placeholder(0, 4, 1).is_some());
    assert!(t.insert_placeholder(5, 9, 2).is_none());
}

#[test]
fn drop_pending_removes_placeholder_keeps_cursor() {
    let mut t = MemoryTree::new("ws", "s1");
    let task = t.insert_placeholder(0, 4, 1).unwrap();
    t.drop_pending(record("aborted"));
    assert!(t.pending.is_none());
    assert!(!t.bundles.contains_key(&task.placeholder_id.0));
    assert_eq!(t.bundled_until, 0); // 游标不动，尾巴留待下次
}

#[test]
fn rebound_counts_consecutive_then_pauses_at_m() {
    let mut t = MemoryTree::new("ws", "s1");
    // 从未整理：直接放行
    assert_eq!(t.on_auto_trigger(5, 3, 3), ReboundDecision::Proceed);
    t.note_organized(5);

    // k=3 轮内连续触发反弹（每次放行后整理收束、基线前移——镜像真实
    // 流程：trigger → organize → note_organized）：第 1、2 次放行，
    // 第 3 次（m）熔断
    assert_eq!(t.on_auto_trigger(8, 3, 3), ReboundDecision::Proceed);
    t.note_organized(8);
    assert_eq!(t.on_auto_trigger(9, 3, 3), ReboundDecision::Proceed);
    t.note_organized(9);
    assert_eq!(t.on_auto_trigger(10, 3, 3), ReboundDecision::Pause);
    assert!(t.paused_auto);
}

#[test]
fn healthy_gap_resets_rebound_counter() {
    let mut t = MemoryTree::new("ws", "s1");
    t.note_organized(10);
    assert_eq!(t.on_auto_trigger(11, 3, 3), ReboundDecision::Proceed); // 反弹1
    t.note_organized(11);
    assert_eq!(t.on_auto_trigger(50, 3, 3), ReboundDecision::Proceed); // 间隔越 k → 归零
    t.note_organized(50);
    assert_eq!(t.on_auto_trigger(51, 3, 3), ReboundDecision::Proceed); // 反弹1（非2）
    assert_eq!(t.rebound.count, 1);
    assert!(!t.paused_auto);
}

#[test]
fn manual_organize_resets_pause() {
    let mut t = MemoryTree::new("ws", "s1");
    t.note_organized(1);
    t.on_auto_trigger(2, 3, 1); // m=1：第一次反弹即熔断
    assert!(t.paused_auto);
    t.mark_manual(9);
    assert!(!t.paused_auto);
    assert_eq!(t.rebound.count, 0);
}

#[test]
fn history_is_capped() {
    let mut t = MemoryTree::new("ws", "s1");
    for i in 0..30 {
        let mut r = record("ok");
        r.ts = i;
        t.push_history(r);
    }
    assert_eq!(t.history.len(), 20);
    assert_eq!(t.history[0].ts, 10); // 最旧的被挤掉
}

#[test]
fn bundle_ids_skip_past_max() {
    let mut t = MemoryTree::new("ws", "s1");
    t.apply_organized(vec![bundle("b3", (0, 1))], 1, record("ok"));
    let task = t.insert_placeholder(2, 3, 1).unwrap();
    assert_eq!(task.placeholder_id.0, "b4");
}
