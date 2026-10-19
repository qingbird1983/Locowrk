//! 管道与触发决策：记忆管理器成功落树 / 垃圾输出降级规则切分 / 增量档触发后
//! 反弹熔断（手动不受限且复位）/ batch 档无窗口永不自动触发。

use super::*;

#[tokio::test]
async fn manual_organize_records_recorder_bundles() {
    let root = temp_root("manual");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let session = session();
    let task = org.plan_manual(&session, "s-manual").expect("发起成功");
    assert_eq!(task.reason, OrganizeReason::Manual);
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-manual");
    assert_eq!(tree.chapters.len(), 2);
    assert_eq!(tree.chaptered_until, 6);
    assert!(tree.pending.is_none());
    assert_eq!(tree.history[0].mode, "recorder");
    assert_eq!(tree.history[0].status, "ok");
    assert_eq!(tree.history[0].manager_input, 100);
    assert_eq!(tree.rebound.users_at_last, 3);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn garbage_recorder_output_degrades_to_rule_split() {
    let root = temp_root("degrade");
    let settings = settings_with(serde_json::json!({}), &root);
    // 记忆管理器连续回垃圾（超重试次数），随后管道降级规则切分
    let junk = vec![Ok("我不会".into()); 4];
    let org = organizer(&root, settings, FakeClient::new(junk));
    let session = session();
    let task = org.plan_manual(&session, "s-rule").expect("发起成功");
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-rule");
    assert_eq!(tree.history[0].mode, "rule");
    assert_eq!(tree.history[0].status, "degraded");
    assert_eq!(tree.chaptered_until, 6); // 游标照样推进（尾巴已章节）
                                         // 规则切分铺满全部材料
    let covered: usize = tree
        .chapters
        .values()
        .map(|b| b.event_range.1 - b.event_range.0 + 1)
        .sum();
    assert_eq!(covered, 6);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn incremental_trigger_then_rebound_pauses_auto() {
    let root = temp_root("rebound2");
    let settings = settings_with(
        serde_json::json!({
            "memory": {"organizePacing": "incremental", "organizeChunkTokens": 1},
            "developer": {"organizeReboundK": 1, "organizeReboundM": 1}
        }),
        &root,
    );
    let client = FakeClient::new(vec![Ok(good_reply()), Ok(good_reply())]);
    let org = organizer(&root, Arc::clone(&settings), client);

    // 第 1 轮收束：触发 → 整理（基线 users=3 落账）
    let first = session();
    let task = org.plan_auto(&first, "s-rb").expect("增量档应触发");
    org.clone().run_task(task).await;

    // 第 2 轮收束（k=1 轮内再触发，m=1 → 第一次反弹即熔断）：不 spawn
    let mut session2 = session();
    session2.append(SessionEvent::UserMessage {
        content: "第3轮问题".into(),
        parts: Vec::new(),
    });
    session2.append(SessionEvent::AssistantMessage {
        content: "第3轮回答".into(),
        tool_calls: vec![],
    });
    assert!(
        org.plan_auto(&session2, "s-rb").is_none(),
        "熔断后自动整理应跳过"
    );
    let tree = tree_at(&root, org.settings.as_ref(), "s-rb");
    assert!(tree.paused_auto);

    // 手动整理不受熔断限制且复位（入口现在是开发者面板那颗按钮，不再是 `/整理`）
    let task = org
        .plan_manual(&session2, "s-rb")
        .expect("手动不受熔断限制");
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-rb");
    assert!(!tree.paused_auto);
    assert_eq!(tree.history.last().unwrap().reason, "manual");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn batch_without_window_never_auto_fires() {
    let root = temp_root("nowin");
    let settings = settings_with(serde_json::json!({}), &root); // 缺省 batch + 窗口未知
    let org = organizer(&root, settings, FakeClient::new(vec![]));
    let session = session();
    assert!(org.plan_auto(&session, "s-nowin").is_none());
    let _ = std::fs::remove_dir_all(&root);
}
