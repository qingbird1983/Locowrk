//! 摘要补写树侧用例（队列 Q8c）。
//!
//! 章节一律经**生产出口**造（`build_chapters(…, "rule")` → `apply_organized`），
//! 不手搭结构体字面量：手搭会造出"字段齐全的假树"（`organize/progress/tests.rs`
//! 立的口径），而本批最要证的恰好是「债由 `source` 决定」——只有走生产出口，
//! 那条置位逻辑才在判据覆盖面里。

use crate::chapter::{ChapterDraft, EventId, MemoryChapter, sanitize_draft};
use crate::tree::{MemoryTree, OrganizeRecord};

use super::ChapterSummary;

fn record(from: EventId, to: EventId, chapters: usize) -> OrganizeRecord {
    OrganizeRecord {
        ts: 1,
        reason: "manual".into(),
        mode: "rule".into(),
        status: "ok".into(),
        from,
        to,
        chapters,
        manager_input: 0,
        manager_output: 0,
        duration_ms: 1,
    }
}

/// 落 `count` 条规则切分章节（生产通路：草稿 → 章节 → 收束）。
/// `now_ms` 成为这些章节的 `created_at`，用来造"旧事优先"的时间差。
fn rule_chapters(t: &mut MemoryTree, count: usize, base: EventId, now_ms: i64) {
    let drafts: Vec<ChapterDraft> = (0..count)
        .map(|i| {
            let from = base + (i as EventId) * 10;
            sanitize_draft(
                &format!("话题{}", from),
                &format!("（规则切分）本章节涵盖事件 {from}–{}。", from + 9),
                (from, from + 9),
            )
            .unwrap()
        })
        .collect();
    let last = base + (count as EventId) * 10 - 1;
    let chapters = t.build_chapters(&drafts, "rule", now_ms);
    t.apply_organized(chapters, last, record(base, last, count), None);
}

#[test]
fn rule_chapters_carry_the_debt_from_the_producer_side() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 2, 0, 5);
    assert_eq!(
        t.summary_candidates(8, 2),
        vec!["b1".to_string(), "b2".to_string()]
    );
    // 生产出口自己就把债标上了，编排层不需要再补一句
    assert!(t.chapters["b1"].summary_pending);
    assert_eq!(t.chapters["b1"].summary_attempts, 0);
}

/// **旧事优先**：候选按 `created_at` 升序，不是按编号、更不是新章优先。
/// 这里刻意让后落的那批编号更大而时间更早（b3/b4 的 created_at=3 早于
/// b1/b2 的 5），排序必须按时间而不是按 id。
#[test]
fn candidates_take_the_oldest_debt_first_and_honour_the_cap() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 2, 0, 5); // b1、b2：created_at = 5
    let drafts: Vec<ChapterDraft> = (0..2)
        .map(|i| {
            sanitize_draft(&format!("早事{i}"), "机械摘要", (40 + i * 10, 49 + i * 10)).unwrap()
        })
        .collect();
    let chapters = t.build_chapters(&drafts, "rule", 3); // b3、b4：更早
    t.apply_organized(chapters, 59, record(40, 59, 2), None);
    assert_eq!(
        t.summary_candidates(9, 2),
        vec![
            "b3".to_string(),
            "b4".to_string(),
            "b1".to_string(),
            "b2".to_string()
        ],
        "先按 created_at，同一时刻再按章节编号"
    );
    assert_eq!(
        t.summary_candidates(2, 2),
        vec!["b3".to_string(), "b4".to_string()],
        "一次只补 `max` 章：剩下的债留在池子里等下一轮"
    );
}

/// 三种不该再烧模型的情形都要退出候选池：占位（在途产物）、用户手改过摘要
/// （纪律 5）、尝试次数已到上限（失败落账的意义就在这）。
#[test]
fn candidates_exclude_placeholder_hand_edited_and_exhausted() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 3, 0, 5);

    // ① 用户手改摘要：退出候选池（债留在磁盘上由人管，不再烧模型）
    t.edit_chapter("b1", None, Some("人写的摘要"));
    assert!(t.chapters["b1"].summary_edited_by_user);
    assert!(!t.summary_candidates(9, 2).contains(&"b1".to_string()));

    // ② 到上限：b2 试满 2 次仍在债上，但不再入选
    t.mark_summary_attempts(&["b2"]);
    assert_eq!(
        t.summary_candidates(9, 2),
        vec!["b2".to_string(), "b3".to_string()]
    );
    t.mark_summary_attempts(&["b2"]);
    assert_eq!(t.summary_candidates(9, 2), vec!["b3".to_string()]);
    // 上限放宽一档它就又回来了 = 这是"次数闸"而不是"永久拉黑"；注意它**不改变
    // 排序**——排序只认 created_at + 章节编号，试没试过不参与其中
    assert_eq!(
        t.summary_candidates(9, 3),
        vec!["b2".to_string(), "b3".to_string()]
    );

    // ③ 占位章节不进池（它有整章节替换的另一条路）
    let task = t.insert_placeholder(60, 69, 7).unwrap();
    assert!(t.chapters[&task.placeholder_id.0].pending);
    assert!(!t.summary_candidates(9, 5).contains(&task.placeholder_id.0));
}

/// `0 = 关掉补写`（可配置原则②）两个闸位各自独立生效，且关掉后**不改磁盘状态**：
/// 债还在，重新打开就接着补（关掉一次不该把历史章节永久判死）。
#[test]
fn either_zero_switch_empties_the_pool_without_clearing_the_debt() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 2, 0, 5);
    assert!(t.summary_candidates(0, 2).is_empty(), "每次补写章数 0 = 关");
    assert!(t.summary_candidates(2, 0).is_empty(), "每章尝试上限 0 = 关");
    assert!(t.chapters["b1"].summary_pending, "关掉只是不挑，不擦掉债");
    assert_eq!(t.summary_candidates(2, 2).len(), 2, "打开即恢复");
}

/// **本批最值钱的一条**：补写只换 `summary`，目录里出现的字段（`chapter_id`、
/// `title`）以及章节的身份字段（`event_range`、`created_at`、`source`）逐字段
/// 不变。宿主侧另有一条端到端判据拿 `TreeAssembler::plan().directory` 比对补写
/// 前后的字节——目录字节是前缀缓存的命中前提，这里先把数据面钉死。
#[test]
fn apply_leaves_every_directory_and_identity_field_untouched() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 2, 0, 5);
    let before: Vec<MemoryChapter> = t.chapters.values().cloned().collect();

    let n = t.apply_summaries(&[
        ChapterSummary {
            chapter_id: "b1".into(),
            summary: "新摘要一".into(),
        },
        ChapterSummary {
            chapter_id: "b2".into(),
            summary: "新摘要二".into(),
        },
    ]);
    assert_eq!(n, 2);
    // BTreeMap 的 values() 按 key 序，前后两次迭代同序 ⇒ 逐对比较成立
    for (b, a) in t.chapters.values().zip(&before) {
        assert_eq!(&b.chapter_id, &a.chapter_id, "章节身份不变");
        assert_eq!(
            &b.title, &a.title,
            "标题不变 = 目录行字节不变 = 前缀缓存不断"
        );
        assert_eq!(
            &b.event_range, &a.event_range,
            "区间不变（改它等于改写历史归属）"
        );
        assert_eq!(b.created_at, a.created_at);
        assert_eq!(
            &b.source, &a.source,
            "生成方式仍是 rule：补写不改写它的来历"
        );
        assert_eq!(&b.expanded_at, &a.expanded_at);
        assert_eq!(&b.collapsed_at, &a.collapsed_at);
        assert_eq!(&b.title_edited_by_user, &a.title_edited_by_user);
        // 无人读的字段不填（模块头注第 2 条）
        assert_eq!(&b.keywords, &a.keywords);
        assert_eq!(b.token_count, a.token_count);
        assert_eq!(
            b.summary,
            if b.chapter_id.0 == "b1" {
                "新摘要一"
            } else {
                "新摘要二"
            }
        );
        assert!(!b.summary_pending, "落笔即清债");
        assert_eq!(&b.summary_attempts, &a.summary_attempts, "补写成功不涨计数");
    }
}

/// 三种"不写字但清债"的跳过：未知 id 什么都不动；占位章节不写（收束时整
/// 章节替换）；用户手改过的不覆盖（纪律 5）**但债要清**——挂着债就是每轮
/// 重烧模型去打一个不该打的字段。
#[test]
fn apply_clears_the_debt_even_when_it_refuses_to_write() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 2, 0, 5);
    t.edit_chapter("b1", None, Some("人写的摘要"));
    let task = t.insert_placeholder(60, 69, 7).unwrap();

    let n = t.apply_summaries(&[
        ChapterSummary {
            chapter_id: "bX".into(),
            summary: "编的".into(),
        },
        ChapterSummary {
            chapter_id: "b1".into(),
            summary: "机器想改人写的".into(),
        },
        ChapterSummary {
            chapter_id: task.placeholder_id.0.clone(),
            summary: "打在占位上".into(),
        },
    ]);
    assert_eq!(n, 0, "三条一条都没写进正文");
    assert_eq!(
        t.chapters["b1"].summary, "人写的摘要",
        "纪律 5：自动层不覆盖人写的"
    );
    assert!(
        !t.chapters["b1"].summary_pending,
        "人已经答过 = 债结清，不再重烧"
    );
    assert!(
        !t.chapters.contains_key("bX"),
        "未知 id 既不该写也不该造出新章节"
    );
    let ph = &t.chapters[&task.placeholder_id.0];
    assert_eq!(
        ph.summary, "整理进行中（占位）。",
        "占位章节等整章节替换，补写不插手"
    );
    assert_eq!(t.chapter_count(), 3, "一次补写不增不减章节");
}

/// 失败落账：计数只涨不清债（否则下一轮它就不在池子里"看起来已补过"，
/// 而磁盘上仍是那句机械摘要 = 假状态）。饱和加防极端配置下溢出。
#[test]
fn attempts_account_for_failure_without_clearing_the_debt() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 1, 0, 5);
    assert_eq!(t.mark_summary_attempts(&["b1", "bX"]), 1, "未知 id 不命中");
    let b = &t.chapters["b1"];
    assert_eq!(b.summary_attempts, 1);
    assert!(
        b.summary_pending,
        "失败不落清债——债还在，等下次机会或到上限"
    );
    assert!(b.summary.starts_with("（规则切分）"), "失败也不改正文");

    t.mark_summary_attempts(&["b1"]);
    t.mark_summary_attempts(&["b1"]);
    // 首次那条 + 这里两次 = 3。计数只累加、不因为"到没到上限"而停下来——
    // 上限是候选池的入口闸，不是计数器的闸（观测面要看到它一共试过几次）。
    assert_eq!(t.chapters["b1"].summary_attempts, 3);
    t.chapters.get_mut("b1").unwrap().summary_attempts = u32::MAX;
    t.mark_summary_attempts(&["b1"]);
    assert_eq!(t.chapters["b1"].summary_attempts, u32::MAX, "饱和加不溢出");
}

/// 补写后的债与计数随投影落盘再读回（serde default 零迁移：Q8c 之前写的
/// 投影没有这两个键，读回 = 无债 / 0 次，由 `chapter.rs` 那条用例钉住）。
#[test]
fn summary_debt_roundtrips_through_the_projection() {
    let mut t = MemoryTree::new("ws", "s1");
    rule_chapters(&mut t, 2, 0, 5);
    t.mark_summary_attempts(&["b2"]);
    t.apply_summaries(&[ChapterSummary {
        chapter_id: "b1".into(),
        summary: "已补".into(),
    }]);

    let json = serde_json::to_string(&t).unwrap();
    let back: MemoryTree = serde_json::from_str(&json).unwrap();
    assert_eq!(back.chapters["b1"].summary, "已补");
    assert!(!back.chapters["b1"].summary_pending);
    assert!(back.chapters["b2"].summary_pending);
    assert_eq!(back.chapters["b2"].summary_attempts, 1);
    assert_eq!(back.summary_candidates(9, 2), vec!["b2".to_string()]);
}
