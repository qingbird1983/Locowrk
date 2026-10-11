//! M4 模式 B 窗口溢出整理（不记反弹 / 永不熔断 / 让位在途任务）+ 自动收起泄压。

use super::*;

// ---- M4 模式 B 溢出整理（不记反弹 / 熔断不受限）+ 自动收起 ----

// （v2.137 起一切会话皆树，tree_organizer 与 organizer 同形，保留别名语义）

#[tokio::test]
async fn window_overflow_organizes_orphans_without_rebound() {
    let root = temp_root("overflow");
    let settings = settings_with(
        serde_json::json!({
            "memory": {"tailStrategy": "window", "tailWindowTurns": 1},
            "developer": {"organizeReboundK": 1, "organizeReboundM": 1}
        }),
        &root,
    );
    let client = FakeClient::new(vec![Ok(good_reply()), Ok(good_reply())]);
    let org = tree_organizer(&root, Arc::clone(&settings), client);

    // 轮 1：chaptered_until=0 < 窗口起点（22）→ 溢出整理只归孤儿 [0..22)
    let s = long_session();
    let task = org.plan_auto(&s, "s-ov").expect("孤儿轮在场应触发溢出整理");
    assert_eq!(task.reason, memory_tree::OrganizeReason::WindowOverflow);
    assert_eq!(task.items.first().unwrap().event, 0);
    assert_eq!(
        task.items.last().unwrap().event,
        21,
        "只归窗口滑出的部分，最后一轮留尾巴"
    );
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-ov");
    assert_eq!(tree.chaptered_until, 22, "游标推进到窗口起点");
    assert_eq!(tree.rebound.users_at_last, 0, "溢出整理不碰反弹基线");
    assert!(!tree.paused_auto);

    // 轮 2：窗口再滑一行——但本轮只有既有 24 事件（无新轮），窗口起点
    // 仍在 22 = 游标处，不触发
    assert!(org.plan_auto(&s, "s-ov").is_none(), "无新孤儿不触发");

    // 加一轮：窗口起点滑到 24 > 游标 22 → 再溢出（连续第 2 次也不熔断——
    // k/m 只惩罚阈值/增量档的烧钱反弹）
    let mut s2 = s;
    s2.append(SessionEvent::UserMessage {
        content: "第12轮问题".into(),
        parts: Vec::new(),
    });
    s2.append(SessionEvent::AssistantMessage {
        content: "第12轮回答".into(),
        tool_calls: vec![],
    });
    let task = org.plan_auto(&s2, "s-ov").expect("新孤儿再触发");
    assert_eq!(task.reason, memory_tree::OrganizeReason::WindowOverflow);
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-ov");
    assert_eq!(tree.chaptered_until, 24);
    assert!(!tree.paused_auto, "溢出整理永不触发熔断");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn overflow_respects_pending_and_legacy_mode() {
    let root = temp_root("ovgates");
    let settings = settings_with(
        serde_json::json!({"memory": {"tailStrategy": "window", "tailWindowTurns": 1}}),
        &root,
    );
    let client = FakeClient::new(vec![]);
    let org = tree_organizer(&root, Arc::clone(&settings), client);
    let s = long_session();

    // 在途任务占位：溢出让位（单写者）
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-ov2");
    tree.insert_placeholder(0, 5, 1).unwrap();
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();
    assert!(org.plan_auto(&s, "s-ov2").is_none(), "在途任务期间不触发");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn auto_collapse_relieves_oldest_expanded() {
    let root = temp_root("autocollapse");
    // 窗口经 llm 档给出（v2.137 自动收起走 session_window 四层链——
    // 模型行缺席时回落档位 window_tokens，但要求模型名非空）；
    // thresholdTokens 900：把「自动整理」触发线抬到本测锚点之上——本测
    // 只验收起，若 600 也过整理线，after_turn 会经 tauri::async_runtime
    // （独立全局 runtime，不受测试单线程 tokio 约束）派生整理任务，
    // FakeClient 空队列秒失败走规则降级、重建章节抹掉 collapsed_at 回写，
    // 与本测读盘竞速（真实 flaky 根因，2026-10-06 定位）。
    let settings = settings_with(
        serde_json::json!({
            "llm": {"active": "p1", "providers": [
                {"id": "p1", "label": "t", "kind": "openai", "model": "m",
                 "windowTokens": 1000, "enabled": true}]},
            "context": {"thresholdTokens": 900},
            "memory": {"collapseEnabled": true, "collapseThresholdPct": 50}
        }),
        &root,
    );
    let client = FakeClient::new(vec![]);
    let org = tree_organizer(&root, Arc::clone(&settings), client);

    // 树 + 两个已展开章节（b2 更早展开）
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-ac");
    let d1 = vec![memory_tree::sanitize_draft("章节一", "摘要一", (0, 1)).unwrap()];
    let chapters = tree.build_chapters(&d1, "recorder", 1);
    tree.apply_organized(
        chapters,
        1,
        memory_tree::OrganizeRecord {
            ts: 1,
            reason: "manual".into(),
            mode: "recorder".into(),
            status: "ok".into(),
            from: 0,
            to: 1,
            chapters: 1,
            manager_input: 0,
            manager_output: 0,
            duration_ms: 1,
        },
        None,
    );
    tree.mark_expanded("b1", 100);
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();

    // 占用未达阈值：不收
    let mut session = long_session();
    session.events().iter().for_each(|_| {});
    org.after_turn(&session, "s-ac");
    let t = tree_at(&root, org.settings.as_ref(), "s-ac");
    assert!(t.chapters["b1"].collapsed_at.is_none(), "占用低不收");

    // 用量锚点来自最后一条 LlmUsage（600 / 1000 = 60% ≥ 50%）→ 收起 b1
    session.append(SessionEvent::LlmUsage {
        input: 600,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: 610,
        duration_ms: 1,
    });
    org.after_turn(&session, "s-ac");
    let t = tree_at(&root, org.settings.as_ref(), "s-ac");
    assert!(
        t.chapters["b1"].collapsed_at.is_some(),
        "过阈值收起最早展开"
    );
    assert_eq!(t.oldest_expanded(), None);

    // 无树会话零参与（v2.137 起一切会话皆树，本行退化为空投影冒烟）
    let other_org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    other_org.after_turn(&session, "s-other");
    let _ = std::fs::remove_dir_all(&root);
}
