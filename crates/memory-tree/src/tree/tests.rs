//! tree 单测：占位章节生命周期 / 反弹熔断 k/m / 历史封顶。

use super::*;

fn chapter(id: &str, range: (EventId, EventId)) -> MemoryChapter {
    MemoryChapter {
        chapter_id: ChapterId(id.into()),
        parent: None,
        children: Vec::new(),
        title: format!("章节{id}"),
        title_edited_by_user: false,
        summary: "摘要".into(),
        summary_edited_by_user: false,
        event_range: range,
        created_at: 1,
        keywords: Vec::new(),
        token_count: 0,
        pending: false,
        source: "recorder".into(),
        expanded_at: None,
        collapsed_at: None,
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
        chapters: 1,
        manager_input: 100,
        manager_output: 50,
        duration_ms: 10,
    }
}

#[test]
fn placeholder_then_apply_replaces_and_advances() {
    let mut t = MemoryTree::new("ws", "s1");
    let task = t.insert_placeholder(10, 19, 5).expect("任务可建");
    assert_eq!(t.chapter_count(), 1);
    assert!(t.chapters[&task.placeholder_id.0].pending);

    t.apply_organized(
        vec![chapter("b1", (10, 14)), chapter("b2", (15, 19))],
        19,
        record("ok"),
    );
    assert!(t.pending.is_none());
    assert_eq!(t.chapter_count(), 2); // 占位章节已撤
    assert_eq!(t.chaptered_until, 20);
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
    assert!(!t.chapters.contains_key(&task.placeholder_id.0));
    assert_eq!(t.chaptered_until, 0); // 游标不动，尾巴留待下次
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
fn chapter_ids_skip_past_max() {
    let mut t = MemoryTree::new("ws", "s1");
    t.apply_organized(vec![chapter("b3", (0, 1))], 1, record("ok"));
    let task = t.insert_placeholder(2, 3, 1).unwrap();
    assert_eq!(task.placeholder_id.0, "b4");
}

#[test]
fn expand_collapse_lifecycle_and_auto_pick() {
    let mut t = MemoryTree::new("ws", "s1");
    t.apply_organized(
        vec![chapter("b1", (0, 4)), chapter("b2", (5, 9))],
        9,
        record("ok"),
    );

    // 未展开不可收起；展开后可收起；再展开解除收起
    assert!(!t.mark_collapsed("b1", 10), "从未展开不可收");
    assert!(!t.mark_expanded("nope", 10), "未知章节拒绝");
    assert!(t.mark_expanded("b2", 10));
    assert!(t.mark_expanded("b1", 20));
    assert_eq!(t.oldest_expanded().as_deref(), Some("b2"), "最早展开优先");
    assert!(t.mark_collapsed("b2", 30));
    assert!(t.chapters["b2"].collapsed_at.is_some());
    assert_eq!(t.oldest_expanded().as_deref(), Some("b1"), "已收起不再入选");
    assert!(t.mark_expanded("b2", 40), "再展开解除收起");
    assert!(t.chapters["b2"].collapsed_at.is_none());
    assert!(t.collapsed_ids().is_empty());

    // 占位章节不可展开 / 收起
    let task = t.insert_placeholder(20, 29, 50).unwrap();
    assert!(!t.mark_expanded(&task.placeholder_id.0, 60));
    assert!(!t.mark_collapsed(&task.placeholder_id.0, 60));
}

#[test]
fn bundle_fields_serde_default_backcompat() {
    // 旧投影文件无 expanded_at / collapsed_at 字段 = None（零迁移）
    let raw = r#"{"chapter_id":"b1","parent":null,"children":[],"title":"t","title_edited_by_user":false,"summary":"s","summary_edited_by_user":false,"event_range":[0,1],"created_at":1,"keywords":[],"token_count":0,"pending":false,"source":"recorder"}"#;
    let b: MemoryChapter = serde_json::from_str(raw).unwrap();
    assert_eq!(b.expanded_at, None);
    assert_eq!(b.collapsed_at, None);
}

#[test]
fn old_projection_with_bundle_era_keys_loads_via_aliases() {
    // 术语统一（2026-10-06）前的投影 JSON：bundles / bundled_until /
    // bundle_id / recorder_input / recorder_output 旧键名必须经 serde
    // alias 读回，绝不因改名丢历史（坏投影当不存在 = 数据丢失，禁止）。
    let raw = r#"{
        "schema_version": 1,
        "workspace": "ws",
        "session_id": "s1",
        "root": "root",
        "bundles": {
            "b1": {"bundle_id":"b1","parent":null,"children":[],"title":"旧章节","title_edited_by_user":false,"summary":"旧摘要","summary_edited_by_user":false,"event_range":[0,3],"created_at":1,"keywords":[],"token_count":0,"source":"recorder"}
        },
        "bundled_until": 4,
        "history": [
            {"ts":1,"reason":"manual","mode":"recorder","status":"ok","from":0,"to":3,"bundles":2,"recorder_input":120,"recorder_output":60,"duration_ms":9}
        ]
    }"#;
    let t: MemoryTree = serde_json::from_str(raw).unwrap();
    assert_eq!(t.chapters.len(), 1, "bundles 旧键读回 chapters");
    assert_eq!(t.chapters["b1"].chapter_id.0, "b1", "bundle_id 旧键读回");
    assert_eq!(t.chaptered_until, 4, "bundled_until 旧键读回");
    assert_eq!(t.history[0].chapters, 2, "台账 bundles 旧键读回");
    assert_eq!(t.history[0].manager_input, 120, "recorder_input 旧键读回");
    assert_eq!(t.history[0].manager_output, 60, "recorder_output 旧键读回");
}
