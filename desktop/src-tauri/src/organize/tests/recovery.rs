//! 崩溃恢复（记忆树 §5.3）：投影带 pending 时重投续跑 / 会话文件已删则放弃并撤占位。

use super::*;

#[tokio::test]
async fn crash_recovery_redispatches_pending_task() {
    let root = temp_root("recover");
    let settings = settings_with(serde_json::json!({}), &root);
    // 会话落盘（模拟上次运行崩在整理中途：投影带 pending）
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let session = session();
    crate::session_log::append_slice(&sessions_dir, "s-crash", &session, 0);
    let store = TreeStore::new(&root);
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-crash");
    tree.insert_placeholder(0, 5, 1).unwrap();
    store.save(&tree).unwrap();

    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let task = org.plan_recovery("s-crash").expect("在途任务应重投");
    assert_eq!(task.reason, OrganizeReason::Recovery);
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-crash");
    assert!(tree.pending.is_none());
    assert_eq!(tree.chaptered_until, 6);
    assert_eq!(tree.history[0].reason, "recovery");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn recovery_gives_up_when_session_file_deleted() {
    let root = temp_root("gone");
    let settings = settings_with(serde_json::json!({}), &root);
    let store = TreeStore::new(&root);
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-gone");
    tree.insert_placeholder(0, 5, 1).unwrap();
    store.save(&tree).unwrap();

    let org = organizer(&root, settings, FakeClient::new(vec![]));
    assert!(org.plan_recovery("s-gone").is_none());
    let tree = tree_at(&root, org.settings.as_ref(), "s-gone");
    assert!(tree.pending.is_none()); // 占位撤除
    assert_eq!(tree.history[0].status, "aborted");
    let _ = std::fs::remove_dir_all(&root);
}
