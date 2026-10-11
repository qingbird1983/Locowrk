//! 轮级总结（Organize Summary Design v1 §3.3）：收束落投影 → 下一轮开场排空落日志并同步
//! 磁盘镜像 → 幂等再排空；降级运行不落总结，事件位越界的存量投影跳过不投。

use super::*;

// ---- 轮级总结（Organize Summary Design v1 §3.3）----

#[tokio::test]
async fn recorder_summary_stored_then_delivered_at_next_turn_opening() {
    let root = temp_root("summary");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(
        &root,
        Arc::clone(&settings),
        FakeClient::new(vec![Ok(good_reply_with_summary())]),
    );
    // 管道收束：总结落树投影
    let task = org.plan_manual(&session(), "s-sum").expect("发起成功");
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-sum");
    assert_eq!(tree.summaries.len(), 1);
    assert!(tree.summaries[0].text.contains("共享 runtime"));

    // 下一轮开场排空：日志落 ContextSummary（断层缝合处 = 游标位 6），
    // 磁盘镜像同步（基座先落盘 = 生产形态：run_turn 每 step 持久化）
    let mut session = session();
    crate::session_log::append_slice(&root.join("sessions"), "s-sum", &session, 0);
    let delivered = org.deliver_summaries(&mut session, "s-sum");
    assert_eq!(delivered.len(), 1);
    assert_eq!(session.len(), 7);
    let msgs = session.derive_messages_from(6);
    assert_eq!(msgs.len(), 1);
    assert!(
        matches!(&msgs[0], session_store::Message::User { content, .. }
            if content.contains("context summary") && content.contains("共享 runtime"))
    );
    let (disk, _) = session_store::load_session(
        &session_store::session_file(&root.join("sessions"), "s-sum").unwrap(),
    )
    .unwrap();
    assert_eq!(disk.len(), 7, "磁盘镜像与内存索引对齐");

    // 幂等：再次排空 = 零新交付（已交付判定自愈）
    let again = org.deliver_summaries(&mut session, "s-sum");
    assert!(again.is_empty());
    assert_eq!(session.len(), 7);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn degraded_run_carries_no_summary_and_truncated_base_skips() {
    let root = temp_root("summary-none");
    let settings = settings_with(serde_json::json!({}), &root);
    // 降级：规则切分无总结
    let junk = vec![Ok("不会".into()); 4];
    let org = organizer(&root, settings, FakeClient::new(junk));
    let task = org.plan_manual(&session(), "s-none").expect("发起成功");
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-none");
    assert!(tree.history[0].status == "degraded");
    assert!(tree.summaries.is_empty(), "宁缺勿假：降级不落总结");
    let mut session = session();
    assert!(org.deliver_summaries(&mut session, "s-none").is_empty());

    // 底座事件位超出日志（截断重写形态）：跳过不投
    let mut t = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-none");
    t.apply_organized(
        vec![memory_tree::MemoryChapter::from_draft(
            memory_tree::ChapterId("b1".into()),
            &memory_tree::sanitize_draft("题", "要", (0, 1)).unwrap(),
            "recorder",
            1,
        )],
        99,
        memory_tree::OrganizeRecord {
            ts: 1,
            reason: "manual".into(),
            mode: "recorder".into(),
            status: "ok".into(),
            from: 0,
            to: 99,
            chapters: 1,
            manager_input: 0,
            manager_output: 0,
            duration_ms: 1,
        },
        Some("越界总结".into()),
    );
    memory_tree::TreeStore::new(&root).save(&t).unwrap();
    let org2 = organizer(
        &root,
        settings_with(serde_json::json!({}), &root),
        FakeClient::new(vec![]),
    );
    assert!(org2.deliver_summaries(&mut session, "s-none").is_empty());
    assert_eq!(session.len(), 6, "越界总结不追加");
    let _ = std::fs::remove_dir_all(&root);
}
