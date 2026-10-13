//! 在途期补触发（《Context Budget Closure》§7.1/§8 · 队列 Q6）：整理在跑时新到的
//! 触发**记进投影标记**而不是蒸发，任务收束后在 `inflight` 释放**之前**串行补跑；
//! 连续补跑有上限（`developer.organizeMaxRerun`，缺省 3），上限账只在**新的自然触发**
//! 时复位；熔断暂停的会话不记标记（补跑不是绕过 `paused_auto` 的后门，§8.4）。
//!
//! 两把档位分开测：`incremental` 档的命中条件是**尾巴增量 token**，`batch` 档才是
//! **anchor 绝对阈值**——分开测是因为在途支必须与常规支共用同一个 `should_auto_organize`
//! （变异 M7 打的是"在途支退成只看 anchor"，只有增量档那条会红）。

use super::*;

/// 让尾巴能命中增量档的最小设置（chunk=1 ⇒ 尾巴非空即触发）。
fn inc_settings(root: &std::path::Path) -> Arc<SettingsStore> {
    settings_with(
        serde_json::json!({"memory": {"organizePacing": "incremental", "organizeChunkTokens": 1}}),
        root,
    )
}

/// 投影里放一个在途任务（形状 = `pipeline` 起手登记的那份占位）。
fn put_pending(root: &std::path::Path, session_id: &str) {
    let store = TreeStore::new(root);
    let mut tree = store
        .load(r"F:\ws\demo", session_id)
        .unwrap_or_else(|| memory_tree::MemoryTree::new(r"F:\ws\demo", session_id));
    tree.insert_placeholder(0, 5, 1).unwrap();
    store.save(&tree).unwrap();
}

#[tokio::test]
async fn in_flight_trigger_marks_needed_instead_of_being_dropped() {
    let root = temp_root("mark");
    let org = organizer(&root, inc_settings(&root), FakeClient::new(vec![]));
    put_pending(&root, "s-mark");
    // 在途 + 尾巴已达增量档 ⇒ 改前这里是一句 `return None`，什么也不留
    assert!(
        org.plan_auto(&session(), "s-mark").is_none(),
        "在途期不发起第二次整理"
    );
    let tree = tree_at(&root, org.settings.as_ref(), "s-mark");
    assert!(
        tree.organize_needed,
        "在途期命中的触发必须记进投影（Q6 §7.1①）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn in_flight_trigger_below_threshold_records_nothing() {
    let root = temp_root("mark-none");
    // batch 档 + 缺省 100K 阈值：测试会话的 anchor 远不到 ⇒ 在途支也不许"永远记"
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![]));
    put_pending(&root, "s-quiet");
    assert!(org.plan_auto(&session(), "s-quiet").is_none());
    let tree = tree_at(&root, org.settings.as_ref(), "s-quiet");
    assert!(
        !tree.organize_needed,
        "没达触发条件就不许记标记（否则标记会变成噪声）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn paused_auto_is_not_a_backdoor_for_rerun() {
    let root = temp_root("paused");
    let org = organizer(&root, inc_settings(&root), FakeClient::new(vec![]));
    let store = TreeStore::new(&root);
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-paused");
    tree.insert_placeholder(0, 5, 1).unwrap();
    tree.paused_auto = true; // 反弹熔断已命中
    store.save(&tree).unwrap();
    assert!(org.plan_auto(&session(), "s-paused").is_none());
    let tree = tree_at(&root, org.settings.as_ref(), "s-paused");
    assert!(
        !tree.organize_needed,
        "熔断暂停期间记标记 = 给暂停中的会话排了一次自动续跑（§8.4 立的是补跑不是后门）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn converged_task_reruns_without_waiting_for_next_turn() {
    let root = temp_root("rerun");
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    // 磁盘上是 12 事件的长会话（在途期用户又聊了六轮），首跑只吃前 6 事件的材料：
    // 收束后游标 = 6 ⇒ 尾巴 [6..12) 非空 ⇒ 补跑必须立刻再整理一次
    crate::session_log::append_slice(&sessions_dir, "s-rerun", &long_session(), 0);
    let org = organizer(
        &root,
        inc_settings(&root),
        FakeClient::new(vec![Ok(good_reply()), Ok(good_reply())]),
    );
    let task = org.plan_manual(&session(), "s-rerun").expect("发起成功");
    // 在途期攒下的那次触发（等价于上一条用例的形状）：手工落标记，然后跑完整管道
    let store = TreeStore::new(&root);
    let mut tree = store.load(r"F:\ws\demo", "s-rerun").unwrap();
    tree.organize_needed = true;
    store.save(&tree).unwrap();

    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-rerun");
    assert_eq!(
        tree.history.len(),
        2,
        "收束后应当场补跑一次，而不是等下一轮 after_turn"
    );
    assert_eq!(tree.rerun_streak, 1);
    assert!(!tree.organize_needed, "标记已消费");
    assert!(tree.pending.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn rerun_stops_at_the_budget_and_keeps_the_mark() {
    let root = temp_root("budget");
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    crate::session_log::append_slice(&sessions_dir, "s-budget", &long_session(), 0);
    let settings = settings_with(
        serde_json::json!({
            "memory": {"organizePacing": "incremental", "organizeChunkTokens": 1},
            "developer": {"organizeMaxRerun": 1}
        }),
        &root,
    );
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let task = org.plan_manual(&session(), "s-budget").expect("发起成功");
    let store = TreeStore::new(&root);
    let mut tree = store.load(r"F:\ws\demo", "s-budget").unwrap();
    tree.organize_needed = true;
    tree.rerun_streak = 1; // 已经补过一次，上限 = 1 ⇒ 这一次不该再跑
    store.save(&tree).unwrap();

    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-budget");
    assert_eq!(
        tree.history.len(),
        1,
        "到上限还补跑 = 模型全挂时形成无界循环（§12.2）"
    );
    assert!(
        tree.organize_needed,
        "熔断只挡这一串续跑，不该把已记账的触发再丢掉"
    );
    assert_eq!(tree.rerun_streak, 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn old_projection_without_the_new_fields_still_loads() {
    // 两个新字段是**加在已有投影上**的（v2.141 `summaries` 同族先例：`serde(default)`
    // = 零迁移，不抬 schema_version）。旧文件里没有它们 ⇒ 必须照样读得回来，
    // 读不回来就是"升级后每个项目的树都打不开"那种事故形状（变异 M9 打这条）。
    let root = temp_root("oldproj");
    let store = TreeStore::new(&root);
    store
        .save(&memory_tree::MemoryTree::new(r"F:\ws\demo", "s-old"))
        .unwrap();
    // 不猜 `scope_dir` 的实现：写完之后在数据根里找出那份投影，把两个新键**删掉**，
    // 就得到"升级前的磁盘形状"（路径由 `TreeStore` 自己决定，测试跟着它走才不会被它的
    // 目录改名顺手打断）。
    let file = find_projection(&root, "s-old.json");
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let obj = v.as_object_mut().unwrap();
    assert!(
        obj.remove("organize_needed").is_some(),
        "本批字段确实写在投影里"
    );
    assert!(obj.remove("rerun_streak").is_some());
    std::fs::write(&file, v.to_string()).unwrap();

    let org = organizer(&root, inc_settings(&root), FakeClient::new(vec![]));
    let loaded = store.load(r"F:\ws\demo", "s-old");
    assert!(
        loaded.is_some(),
        "旧投影必须仍可读（打不开 = 升级把每个项目的树都废了）"
    );
    let t = loaded.unwrap();
    assert!(!t.organize_needed, "缺键 = 没有攒下的触发（default 语义）");
    assert_eq!(t.rerun_streak, 0);
    // 顺带证明宿主侧读的是同一个形状
    let via_org = org.plan_manual(&session(), "s-old");
    assert!(
        via_org.is_ok(),
        "旧投影上的会话仍可正常发起整理: {:?}",
        via_org.err()
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 在数据根里找出某个投影文件（不让测试去猜 `TreeStore` 的目录布局）。
fn find_projection(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .file_name()
                .map(|n| n.to_string_lossy() == name)
                .unwrap_or(false)
            {
                return p;
            }
        }
    }
    panic!("投影文件没找到: {name}");
}

#[tokio::test]
async fn natural_trigger_resets_the_rerun_budget() {
    let root = temp_root("reset");
    let org = organizer(
        &root,
        inc_settings(&root),
        FakeClient::new(vec![Ok(good_reply())]),
    );
    let store = TreeStore::new(&root);
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-reset");
    tree.organize_needed = true;
    tree.rerun_streak = 2;
    store.save(&tree).unwrap();

    // 新的自然触发（after_turn）到了 = 整理追得上对话节奏 ⇒ 账重新起算。
    // 断言只看同步效果（clear 发生在 spawn **之前**），异步那一程不归这条判据管。
    let arc = org.clone();
    arc.after_turn(&session(), "s-reset");
    let tree = tree_at(&root, org.settings.as_ref(), "s-reset");
    assert_eq!(
        tree.rerun_streak, 0,
        "上限账若不在自然触发时清，补跑会自己把账抹不掉也抹不满"
    );
    assert!(
        !tree.organize_needed,
        "本轮材料是从游标起的整条尾巴，被标记的那次天然包含在内"
    );
    let _ = std::fs::remove_dir_all(&root);
}
