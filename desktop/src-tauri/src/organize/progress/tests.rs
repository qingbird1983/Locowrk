//! 常驻进展行（MR 稿 §4.2 · 队列 Q2）判据。
//!
//! 夹具照 `memory_files/tests.rs` 的既有构造写法抄（`build_chapters` +
//! `apply_organized` + `insert_placeholder`）——那是生产整理收束后树的本体形状，
//! 手搭 `MemoryChapter { … }` 会造出"字段齐全的假树"（Q1b 立的口径）。
//!
//! 四档对应稿 §4.5 的验收：有树 / 空树（含"只有占位章节"）/ 400 字符钳制 /
//! 会话内二次请求字节不变（后者在 `assemble/tests.rs` 走生产 `plan` 判，
//! 这里只钉钉死表自身的机制）。

use memory_tree::{ChapterDraft, MemoryTree, OrganizeRecord};

use super::*;

const NOW: i64 = 1_700_000_000_000;

/// 实章节树：`titles` 各成一章（区间连续、创建序 = 传入序）。
fn real_tree(ws: &str, sid: &str, titles: &[&str], created_ms: i64) -> MemoryTree {
    let mut t = MemoryTree::new(ws, sid);
    let drafts: Vec<ChapterDraft> = titles
        .iter()
        .enumerate()
        .map(|(i, title)| ChapterDraft {
            title: (*title).into(),
            summary: format!("摘要{i}"),
            event_range: (i * 3, i * 3 + 2),
        })
        .collect();
    let n = drafts.len();
    let chapters = t.build_chapters(&drafts, "recorder", created_ms);
    t.apply_organized(
        chapters,
        n * 3,
        OrganizeRecord {
            ts: created_ms,
            reason: "manual".into(),
            mode: "recorder".into(),
            status: "ok".into(),
            from: 0,
            to: n * 3,
            chapters: n,
            manager_input: 0,
            manager_output: 0,
            duration_ms: 0,
        },
        None,
    );
    t
}

fn row(ws: &str, sid: &str, titles: &[&str], created_ms: i64, mtime: i64) -> Row {
    (
        real_tree(ws, sid, titles, created_ms),
        sid.to_string(),
        mtime,
    )
}

type Row = (MemoryTree, String, i64);

// ---------------------------------------------------------------- 有货 / 没货

#[test]
fn progress_line_reads_out_sessions_chapters_and_the_three_tools() {
    let rows = vec![
        row(
            r"F:\p",
            "s1",
            &["打砖块第一轮", "自动化测试"],
            1000,
            NOW - 86_400_000,
        ),
        row(
            r"F:\p",
            "s2",
            &["Bug1 纯垂直徘徊导致僵局"],
            2000,
            NOW - 3_600_000,
        ),
    ];
    let line = render(&rows, NOW).expect("两棵实树必有货");
    assert!(
        line.starts_with("[project memory]"),
        "段首标记（diagnose 的切片靠稳定标记，别动前缀）: {line}"
    );
    assert!(line.contains("2 organized session(s)"), "{line}");
    assert!(line.contains("3 chapter(s)"), "{line}");
    // 三件指路工具一个都不能少（这一段的存在意义就是让模型知道去哪查）
    for tool in [
        "memory_project_history",
        "memory_search_titles",
        "memory_expand",
    ] {
        assert!(line.contains(tool), "指路缺 {tool}: {line}");
    }
    // 章节标题取到（"最近一份"优先：s2 的 mtime 更新 ⇒ 它的标题必须在）
    assert!(line.contains("Bug1 纯垂直徘徊导致僵局"), "{line}");
    // 最近整理时间用**最新**那份的 mtime（1 小时前，不是 1 天前）
    assert!(line.contains("last organize 1 h ago"), "{line}");
}

#[test]
fn a_tree_with_only_pending_chapters_is_not_stock() {
    // 占位章节 = 整理在途，还没有真内容（`directory_text` 同纪律）。
    // 把它算进"整理过 N 个会话"会让进展行在投影刚建好时就谎报有货。
    let mut t = MemoryTree::new(r"F:\p", "s1");
    t.insert_placeholder(0, 4, 1000).unwrap();
    let rows: Vec<Row> = vec![(t, "s1".into(), NOW)];
    assert!(render(&rows, NOW).is_none(), "只有占位章节不许出货");
}

#[test]
fn an_empty_workspace_produces_no_line() {
    assert!(render(&[], NOW).is_none(), "全新项目 = 一行都不注");
}

#[test]
fn titles_dedup_across_trees() {
    // 同一标题在两棵树里出现（M5 用户编辑 / 规则切分的兜底标题都可能撞）⇒
    // 只列一次，否则进展行看起来像"有更多章节"。
    let rows = vec![
        row(r"F:\p", "s1", &["同一个标题"], 1000, NOW - 60_000),
        row(r"F:\p", "s2", &["同一个标题"], 2000, NOW - 120_000),
    ];
    let line = render(&rows, NOW).unwrap();
    assert_eq!(line.matches("同一个标题").count(), 1, "{line}");
    assert!(
        line.contains("2 organized session(s)"),
        "会话数仍按两棵算: {line}"
    );
}

#[test]
fn only_the_two_freshest_trees_contribute_titles() {
    // 第三份往后的章节对"最近做了什么"没有信息量（那是 project_history 的职责）。
    let rows = vec![
        row(
            r"F:\p",
            "s-old",
            &["最老一季的章节"],
            1000,
            NOW - 30 * 86_400_000,
        ),
        row(
            r"F:\p",
            "s-mid",
            &["中间一季的章节"],
            2000,
            NOW - 2 * 86_400_000,
        ),
        row(
            r"F:\p",
            "s-new",
            &["最新一季的章节"],
            3000,
            NOW - 86_400_000,
        ),
    ];
    let line = render(&rows, NOW).unwrap();
    assert!(line.contains("最新一季的章节"), "{line}");
    assert!(line.contains("中间一季的章节"), "{line}");
    assert!(
        !line.contains("最老一季的章节"),
        "第三份的标题不许进: {line}"
    );
    // 但读数（会话数/章节数）仍算全部——那是"这个项目有多少货"的真账
    assert!(line.contains("3 organized session(s)"), "{line}");
    assert!(line.contains("3 chapter(s)"), "{line}");
}

// ---------------------------------------------------------------- 预算

#[test]
fn progress_line_never_exceeds_the_400_char_budget() {
    // 标题可以是人写的长句子（M5 允许编辑）。超长时必须**砍标题保指路**，
    // 而不是整体溢出——这一段进 system，预算是硬约束（稿 §6）。
    let long = "一".repeat(400);
    let rows = vec![
        row(r"F:\p", "s1", &[&long, &long, &long], 1000, NOW - 60_000),
        row(r"F:\p", "s2", &["短标题"], 2000, NOW - 120_000),
    ];
    let line = render(&rows, NOW).expect("有货");
    assert!(
        line.chars().count() <= MAX_PROGRESS_CHARS,
        "超预算 {} 字符",
        line.chars().count()
    );
    assert!(
        line.contains("memory_project_history"),
        "砍的必须是标题，指路三件要留住: {line}"
    );
    assert!(
        line.contains("2 organized session(s)"),
        "读数也要留住: {line}"
    );
    // 一个标题都放不下时也必须承认有标题被省——静默少列会让模型以为"没章节可看"
    assert!(line.contains("Recent chapters omitted"), "{line}");
}

#[test]
fn some_titles_kept_and_the_rest_marked_omitted() {
    // 中间档：预算装得下部分标题时，剩下的必须以省略标记承认"还有"。
    // 夹具要**三个互不相同**的长标题：相同的会被去重成 1 个候选，于是"部分
    // 省略"这一档根本没被走到（第一版传的是 [&long, &long, "短的"]，去重后剩
    // 2 个、全放得下 ⇒ 一条空转判据，同族 = 台账「空转对照」）。
    let a = "甲".repeat(150);
    let b = "乙".repeat(150);
    let c = "丙".repeat(150);
    let rows = vec![row(r"F:\p", "s1", &[&a, &b, &c], 1000, NOW - 60_000)];
    let line = render(&rows, NOW).unwrap();
    assert!(
        line.chars().count() <= MAX_PROGRESS_CHARS,
        "超预算 {}",
        line.chars().count()
    );
    assert!(line.contains('…'), "省略标记在场: {line}");
    assert!(
        line.contains("Recent chapters:"),
        "部分保留时仍是清单形: {line}"
    );
    assert_eq!(line.matches('“').count(), 1, "只列了 1 个标题: {line}");
    assert_eq!(line.matches('”').count(), 1, "引号成对: {line}");
}

// ---------------------------------------------------------------- 时间形

#[test]
fn human_age_buckets_and_no_negative_clocks() {
    assert_eq!(human_age(NOW, NOW), "just now");
    assert_eq!(human_age(NOW - 5 * 60_000, NOW), "5 min ago");
    assert_eq!(human_age(NOW - 3 * 3_600_000, NOW), "3 h ago");
    assert_eq!(human_age(NOW - 2 * 86_400_000, NOW), "2 days ago");
    // 投影时间戳在未来（时钟回拨 / 别的机器写的文件）：不许发明负数天数
    assert_eq!(human_age(NOW + 86_400_000, NOW), "just now");
}

// ---------------------------------------------------------------- 钉死表

#[test]
fn pin_key_changes_when_the_project_attribution_changes() {
    assert_ne!(
        pin_key("s1", r"f:\p\a"),
        pin_key("s1", r"f:\p\b"),
        "会话改归属 = 换键 = 重算（同记忆块的 (session_id, scope_id) 纪律）"
    );
    assert_ne!(pin_key("s1", ""), pin_key("s2", ""));
}

#[test]
fn pin_freezes_the_line_and_also_caches_the_absence() {
    let mut pin = ProgressPin::default();
    let k = pin_key("s1", "f:\\p");
    assert_eq!(pin.get(&k), None, "未查过 = 未缓存（区别于缓存了 None）");
    pin.insert(k.clone(), None);
    assert_eq!(
        pin.get(&k),
        Some(None),
        "「确实没货」也要缓存：否则每 step 重扫一遍投影目录"
    );
    pin.insert(k.clone(), Some("line".into()));
    assert_eq!(pin.get(&k), Some(Some("line".to_string())), "重算覆盖旧值");
}

#[test]
fn pin_evicts_the_oldest_session_beyond_the_cap() {
    // 保险丝：桌面进程可以开很多会话，钉死表不许无限膨胀。
    let mut pin = ProgressPin::default();
    for i in 0..PIN_CAP + 5 {
        pin.insert(pin_key(&format!("s{i}"), "w"), Some(format!("line{i}")));
    }
    assert_eq!(pin.map.len(), PIN_CAP, "容量封顶");
    assert_eq!(pin.get(&pin_key("s0", "w")), None, "最旧的被驱逐");
    assert_eq!(
        pin.get(&pin_key(&format!("s{}", PIN_CAP + 4), "w")),
        Some(Some(format!("line{}", PIN_CAP + 4))),
        "最新的留着"
    );
}
