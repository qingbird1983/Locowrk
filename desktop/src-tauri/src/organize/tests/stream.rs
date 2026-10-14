//! 整组会话流读取（队列 Q14 · MB §2.3）判据：总结归组 / 未归总结（orphan）虚拟组 /
//! 行号全局连续 / 占位章节排除 / 单节越界只脏自己 / 组键与缺树的拒绝面 /
//! 总结自带 `chapter_ids`（归组权威源在后端）。
//!
//! 夹具口径同 `progress/tests.rs`：树用**生产 API** 搭（`build_chapters` +
//! `apply_organized` + `insert_placeholder`），不手搭 `MemoryChapter { … }`——
//! 后者会造出「字段齐全的假树」，读出来的形状好看但生产根本产不出（Q1b 立的口径）。
//! 会话日志走 `session_log::append_slice` 落盘：`tree_stream` 的原文真源是磁盘
//! 日志（与 `tree_original` 同一条 `rows_between` 通路），内存会话不参与。

use std::sync::Arc;

use memory_tree::{ChapterDraft, MemoryTree, OrganizeRecord};

use super::*;
use crate::organize::stream_view::ORPHAN_GROUP;

const WS: &str = r"F:\ws\demo";

fn run_record(from: usize, to: usize, chapters: usize, ts: i64) -> OrganizeRecord {
    OrganizeRecord {
        ts,
        reason: "manual".into(),
        mode: "recorder".into(),
        status: "ok".into(),
        from,
        to,
        chapters,
        manager_input: 0,
        manager_output: 0,
        duration_ms: 0,
    }
}

fn draft(title: &str, from: usize, to: usize) -> ChapterDraft {
    ChapterDraft {
        title: title.into(),
        summary: format!("{title} 的摘要"),
        event_range: (from, to),
    }
}

/// 一轮整理收束（走生产落树通路）。`summary = None` 即「这一轮没有轮级总结」——
/// 规则切分降级轮的真实形状，也正是 MB §2.2 第 1 条孤儿章节的来源。
fn settle(tree: &mut MemoryTree, drafts: &[ChapterDraft], summary: Option<&str>, ts: i64) {
    let from = drafts.first().map(|d| d.event_range.0).unwrap_or(0);
    let to = drafts.last().map(|d| d.event_range.1).unwrap_or(0);
    let n = drafts.len();
    let chapters = tree.build_chapters(drafts, "recorder", ts);
    tree.apply_organized(
        chapters,
        to,
        run_record(from, to, n, ts),
        summary.map(str::to_string),
    );
}

/// 六事件会话（三轮问答，0..=5）落盘 + 两章一轮树：`b1` 覆盖 0..1 并带轮级总结
/// `0:1`，`b2`(2..3) 与 `b3`(4..5) 属**无总结**的第二轮 ⇒ 两个孤儿章节。
/// 返回编排器（读侧不触模型，假客户端给空脚本）。
fn seeded(root: &std::path::Path, sid: &str) -> Arc<Organizer> {
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    crate::session_log::append_slice(&sessions_dir, sid, &session(), 0);

    let mut tree = MemoryTree::new(WS, sid);
    settle(
        &mut tree,
        &[draft("第一轮", 0, 1)],
        Some("本轮总结。"),
        1_000,
    );
    settle(
        &mut tree,
        &[draft("第二轮", 2, 3), draft("第三轮", 4, 5)],
        None,
        2_000,
    );
    memory_tree::TreeStore::new(root).save(&tree).unwrap();
    let settings = settings_with(serde_json::json!({}), root);
    organizer(root, settings, FakeClient::new(vec![]))
}

#[test]
fn summary_group_returns_only_its_own_chapters() {
    let root = temp_root("stream-run");
    let org = seeded(&root, "s-run");

    let v = org.tree_stream("s-run", WS, "0:1").expect("总结组可读");
    assert_eq!(v.from, 0);
    assert_eq!(v.to, 1);
    assert_eq!(
        v.sections
            .iter()
            .map(|s| s.chapter_id.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
        ["b1"],
        "组内只有区间落在 0..=1 的那一章"
    );
    // 行号 = 会话日志绝对事件位（不是本节从 0 重编）
    assert_eq!(
        v.sections[0]
            .rows
            .iter()
            .map(|r| r.index)
            .collect::<Vec<_>>()
            .as_slice(),
        [0, 1]
    );
    assert_eq!(v.sections[0].rows[0].text, "第0轮问题");
    assert_eq!(v.total_rows, 2);
    assert_eq!(v.sections[0].source, "recorder");
    assert!(!v.sections[0].edited, "没人改过 = 不带手动徽章");
    assert!(v.sections[0].error.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn orphan_group_covers_unassigned_chapters_with_globally_continuous_rows() {
    let root = temp_root("stream-orphan");
    let org = seeded(&root, "s-orphan");

    let v = org
        .tree_stream("s-orphan", WS, ORPHAN_GROUP)
        .expect("未归组可读");
    assert_eq!(v.from, 2);
    assert_eq!(v.to, 5);
    assert_eq!(v.total_rows, 4, "两章各两行（一问一答）");
    let ids: Vec<&str> = v.sections.iter().map(|s| s.chapter_id.as_str()).collect();
    assert_eq!(ids, ["b2", "b3"], "事件序 = 区间升序");
    // 这一串是 MB §2.3「后端拼装」的全部理由：跨节行号连续，前端拿不到第二坐标系
    let rows: Vec<usize> = v
        .sections
        .iter()
        .flat_map(|s| s.rows.iter().map(|r| r.index))
        .collect();
    assert_eq!(
        rows,
        [2, 3, 4, 5],
        "第二节的行号接着第一节往下走，不从 0 重编"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn placeholder_chapter_is_excluded_from_both_groups() {
    let root = temp_root("stream-pending");
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    crate::session_log::append_slice(&sessions_dir, "s-pending", &session(), 0);

    // 在途树：两章两轮（b1 有总结 / b2 无总结）+ 生产占位章节 b3
    let mut tree = MemoryTree::new(WS, "s-pending");
    settle(
        &mut tree,
        &[draft("第一轮", 0, 1)],
        Some("本轮总结。"),
        1_000,
    );
    settle(&mut tree, &[draft("第二轮", 2, 3)], None, 2_000);
    assert!(
        tree.insert_placeholder(4, 5, 3_000).is_some(),
        "占位章节走生产 API"
    );
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![]));

    // 三条通路都不含占位章节：单组、未归组、总结归组字段
    let orphan = org
        .tree_stream("s-pending", WS, ORPHAN_GROUP)
        .expect("未归组可读");
    assert_eq!(
        orphan
            .sections
            .iter()
            .map(|s| s.chapter_id.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
        ["b2"],
        "MB §2.2 第 2 条：会话流不含未定稿章节"
    );
    let run = org.tree_stream("s-pending", WS, "0:1").expect("总结组可读");
    assert_eq!(run.sections.len(), 1);
    let chapters = org.tree_chapters("s-pending", WS);
    assert_eq!(chapters.summaries[0].chapter_ids, ["b1"]);
    assert_eq!(
        chapters.chapters.iter().filter(|b| b.pending).count(),
        1,
        "章节清单仍如实给出占位桩（标题列要显示「整理中…」）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn one_chapter_out_of_the_log_dirties_only_its_own_section() {
    let root = temp_root("stream-slice-err");
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    crate::session_log::append_slice(&sessions_dir, "s-slice", &session(), 0);

    // 一轮两章：b1 正常（0..1），b2 指向日志之外（会话被截断重写过）
    let mut tree = MemoryTree::new(WS, "s-slice");
    settle(
        &mut tree,
        &[draft("还在", 0, 1), draft("越界", 9, 10)],
        Some("一轮两章。"),
        1_000,
    );
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![]));

    let v = org
        .tree_stream("s-slice", WS, "0:10")
        .expect("整组仍应读得出");
    assert_eq!(v.sections.len(), 2, "一节坏掉不撤整组（MB §3.3）");
    assert!(v.sections[0].error.is_none());
    assert_eq!(v.sections[0].rows.len(), 2);
    assert!(
        v.sections[1].error.is_some(),
        "越界节给出行内错误：{}",
        v.sections[1].error.as_deref().unwrap_or("")
    );
    assert!(
        v.sections[1].rows.is_empty(),
        "错误节不得演「这一节没有内容」——空 rows 与 error 同时在场"
    );
    assert_eq!(v.total_rows, 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unknown_group_missing_tree_and_bad_keys_are_rejected_not_empty() {
    let root = temp_root("stream-reject");
    let org = seeded(&root, "s-reject");

    // 组键形状不成立
    assert!(org.tree_stream("s-reject", WS, "abc").is_err());
    assert!(org.tree_stream("s-reject", WS, "5:2").is_err());
    assert!(org.tree_stream("s-reject", WS, "").is_err());
    // 形状成立但本会话总结链里没这个区间 ⇒ 拒，不演空面板
    assert!(org.tree_stream("s-reject", WS, "2:5").is_err());
    // 投影缺失
    assert!(org.tree_stream("s-none", WS, ORPHAN_GROUP).is_err());
    // 会话日志已删：投影在、原文读不出 = Err（整组级失败，不是逐节空 rows）
    std::fs::remove_dir_all(root.join("sessions")).unwrap();
    assert!(org.tree_stream("s-reject", WS, "0:1").is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sections_follow_event_order_even_when_chapter_ids_do_not() {
    let root = temp_root("stream-order");
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    crate::session_log::append_slice(&sessions_dir, "s-order", &session(), 0);
    // 同一轮的两份草稿按**倒序**入树：这里没有占位章节消耗 b1，所以首草稿拿到的
    // id 是 b1 而区间是 4..5，第二份是 b2 而区间是 2..3 ⇒ id 序与事件序故意不一致。
    let mut tree = MemoryTree::new(WS, "s-order");
    settle(
        &mut tree,
        &[draft("后一段", 4, 5), draft("前一段", 2, 3)],
        None,
        1_000,
    );
    memory_tree::TreeStore::new(&root)
        .save(&tree)
        .expect("倒序入树的投影要落盘");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![]));

    let v = org.tree_stream("s-order", WS, ORPHAN_GROUP).unwrap();
    assert_eq!(
        v.sections
            .iter()
            .map(|s| s.chapter_id.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
        ["b2", "b1"],
        "小节按事件区间升序，不按章节 id 的数值序"
    );
    let rows: Vec<usize> = v
        .sections
        .iter()
        .flat_map(|s| s.rows.iter().map(|r| r.index))
        .collect();
    assert_eq!(rows, [2, 3, 4, 5], "汇出来的会话流读起来是对话顺序");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn chapter_ids_come_from_the_projection_and_are_ordered_numerically() {
    let root = temp_root("stream-ids");
    let org = seeded(&root, "s-ids");

    let v = org.tree_chapters("s-ids", WS);
    assert_eq!(v.summaries.len(), 1, "只有一轮有轮级总结");
    assert_eq!(v.summaries[0].event_from, 0);
    assert_eq!(v.summaries[0].event_to, 1);
    assert_eq!(v.summaries[0].chapter_ids, ["b1"]);

    // 项目联合视图的同一字段同一算法（两构造点共用 chapters_in）
    let p = org.tree_project(WS);
    assert_eq!(p.summaries.len(), 1);
    assert_eq!(p.summaries[0].chapter_ids, ["b1"]);
    assert_eq!(p.summaries[0].session_id, "s-ids");
    let _ = std::fs::remove_dir_all(&root);
}
