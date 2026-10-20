//! 摘要债的归属用例（队列 Q8c · v2.194 自 `summary_backfill.rs` 拆出，断言一字未改）。
//!
//! 这一簇只回答一个问题：**谁欠这条债**。规则 = `chapter.rs` 的 `from_draft` 里那一处
//! `summary_pending: source == "rule"`，它覆盖规则切分这一个来源，所以两条用例分别钉：
//! 记录员产物根本不欠债（`recorder_chapters_carry_no_debt`）、记录员降级支走
//! `build_chapters(…, "rule")` 时欠债且会被补上（`the_degraded_recorder_path_…`）。
//!
//! S4 刀① 退役轮内泄压后，原来第三条（泄压产物欠债）与其造债夹具一并删除——泄压
//! 已不是生产通路，「谁欠债」不再有它的份。
//!
//! 共享夹具（`persisted_turn` / `GOOD_SUMMARY_REPLY`）已上收 `tests/mod.rs`：
//! 兄弟簇互相看不见私有项（形状闸 S3），共用东西只能住宿主。

use super::*;

/// 记录员通路产出的章节**不欠债**（`source = "recorder"`）⇒ 补写通路一次调用
/// 都不发起、投影零改写。
///
/// ⚠ 这条第一次跑的红是判据的错不是代码的错：原先用 `plan_manual` + `run_task`
/// 去造 recorder 章节，而夹具给的脚本是 `vec![]`（本意就是"证明没调用模型"），
/// 于是 `run_task` 当场走**降级支**、产出 `source = "rule"` 的欠债章节 ——
/// 断言 "recorder" 必红。既然空脚本与"记录员成功"在这个夹具里互斥，就改用
/// 生产出口 `build_chapters(…, "recorder")` 直接落那批章节（手搭结构体字面量
/// 造不出"债由 source 决定"这件事，所以不走字面量）。
#[tokio::test]
async fn recorder_chapters_carry_no_debt() {
    let root = temp_root("backfill-recorder");
    let settings = settings_with(
        serde_json::json!({"context": {"thresholdTokens": 1_000}}),
        &root,
    );
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let mut s = Session::new();
    for i in 0..3 {
        s.append(SessionEvent::UserMessage {
            content: format!("问{i}"),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("答{i}"),
            tool_calls: vec![],
        });
    }
    crate::session_log::append_slice(&root.join("sessions"), "s-bfr", &s, 0);

    let mut tree = org.load_tree(r"F:\ws\demo", "s-bfr");
    let drafts =
        vec![memory_tree::sanitize_draft("共享 runtime 排查", "记录员写的摘要。", (0, 5)).unwrap()];
    let chapters = tree.build_chapters(&drafts, "recorder", 1);
    tree.apply_organized(
        chapters,
        5,
        Organizer::record(
            OrganizeReason::Manual,
            "recorder",
            "ok",
            0,
            5,
            1,
            memory_tree::MemoryManagerUsage::default(),
            1,
        ),
        None,
    );
    org.tree_store.save(&tree).unwrap();
    assert_eq!(
        tree_at(&root, org.settings.as_ref(), "s-bfr").chapters["b1"].source,
        "recorder"
    );

    let dump = || serde_json::to_string(&tree_at(&root, org.settings.as_ref(), "s-bfr")).unwrap();
    let before = dump();
    // 空脚本夹具：真去调模型就会拿到「没有脚本回复」的 Err，失败记账会把
    // `summary_attempts` 推起来 ⇒ 字符串必然不同。字节不变 = 一次调用都没发生。
    org.backfill_summaries(r"F:\ws\demo", "s-bfr").await;
    assert_eq!(dump(), before, "无债可补 ⇒ 投影零改写、零调用");
    std::fs::remove_dir_all(&root).ok();
}

/// 记录员**降级支**（模型不可用 → 规则切分）留下的同样是欠债章节：它与泄压支
/// 共用 `build_chapters(…, "rule")` 这一个出口，所以 `from_draft` 里那一处置位
/// 覆盖两个来源。这里跑的是完整 `run_task`（不是手工状态），事后台账条数不变
/// 就是「补写不记整理台账」的行为侧凭证。
#[tokio::test]
async fn the_degraded_recorder_path_also_leaves_debt_and_gets_filled() {
    let root = temp_root("backfill-degraded");
    let settings = settings_with(
        serde_json::json!({
            "context": {"thresholdTokens": 1_000},
            "developer": {"organizeSummaryChapters": 6, "organizeSummaryAttempts": 2}
        }),
        &root,
    );
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, _) = persisted_turn(&root, "s-bfd");
    let task = org.plan_manual(&session, "s-bfd").expect("手动整理可发起");
    org.clone().run_task(task).await;

    let tree = tree_at(&root, org.settings.as_ref(), "s-bfd");
    // 章节 id 不写死：`pipeline` 先插占位（占位吃掉一个编号）再落真章节，
    // 这里按"存在一条非占位章节"取它——照记忆写 `b1` 会红一次（本批第一次跑
    // 就红在这里，红的是判据的假设而不是代码的行为）。
    let degraded = tree
        .chapters
        .values()
        .find(|b| !b.pending)
        .expect("降级支应产出章节");
    let degraded_id = degraded.chapter_id.0.clone();
    assert_eq!(degraded.source, "rule", "空脚本 ⇒ 降级规则切分");
    assert!(degraded.summary_pending, "降级支与泄压支同一条置位通路");
    let history_before = tree.history.len();

    let reply = format!(r#"[{{"id":"{degraded_id}","summary":"补上的降级章节摘要。"}}]"#);
    let filler = organizer(
        &root,
        Arc::clone(&settings),
        FakeClient::new(vec![Ok(reply)]),
    );
    filler.backfill_summaries(r"F:\ws\demo", "s-bfd").await;

    let after = tree_at(&root, filler.settings.as_ref(), "s-bfd");
    assert!(after.chapters[&degraded_id]
        .summary
        .contains("补上的降级章节摘要"));
    assert!(!after.chapters[&degraded_id].summary_pending);
    assert_eq!(
        after.history.len(),
        history_before,
        "补写不是又一次整理：台账不加分"
    );
    assert_eq!(after.chaptered_until, tree.chaptered_until, "游标不动");
    std::fs::remove_dir_all(&root).ok();
}
