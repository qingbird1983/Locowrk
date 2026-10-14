//! 开发者观测与人类编辑面：清单 / 章节视图 / 原文切片 / 项目平铺 / 改章节（人改保护 +
//! tree.md 同步）/ 删章节（游标不动）。

use super::*;

#[tokio::test]
async fn view_bundles_and_original_read_from_disk() {
    let root = temp_root("view");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    assert!(org.tree_sessions().is_empty(), "空数据根 = 空清单");

    // 会话落盘 + 手动整理（M3 视图数据源 = 磁盘，与内存会话无关）
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let session = session();
    crate::session_log::append_slice(&sessions_dir, "s-view", &session, 0);
    let disk_path = session_store::session_file(&sessions_dir, "s-view").unwrap();
    let (disk, _) = session_store::load_session(&disk_path).unwrap();
    let task = org.plan_manual(&disk, "s-view").expect("发起成功");
    org.clone().run_task(task).await;

    // 清单：一树两章节，mtime 有值
    let list = org.tree_sessions();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].session_id, "s-view");
    assert_eq!(list[0].chapter_count, 2);
    assert!(list[0].updated_ms > 0);
    assert!(!list[0].pending);
    // 队列 Q14 第三刀：清单侧那个「N 总结」必须与明细视图里的总结条数**同源同数**
    // ——左栏那一行读的是清单，画布读的是明细，两处对不上就是第二把尺。
    let counts = org.tree_chapters("s-view", r"F:\ws\demo");
    assert_eq!(
        list[0].summary_count,
        counts.summaries.len(),
        "清单报的总结数 = 投影里的条数（不是章节数顶替）"
    );
    assert_ne!(
        list[0].summary_count, list[0].chapter_count,
        "两把尺在同一条会话上取值不同，才证明这里没把章节数当总结数抄"
    );

    // 章节清单：数值序 b1、b2（BTreeMap 字典序会把 b10 排到 b2 前）
    let view = org.tree_chapters("s-view", r"F:\ws\demo");
    assert!(view.exists);
    assert_eq!(view.chapters.len(), 2);
    // 占位章节消耗 b1，记忆管理器真章节从 b2 起
    assert_eq!(view.chapters[0].chapter_id, "b2");
    assert_eq!(view.chapters[0].title, "话题一");
    assert_eq!(view.chapters[0].source, "recorder");
    assert!(!view.chapters[0].pending);
    assert_eq!(view.chapters[0].event_from, 0);
    assert_eq!(view.chapters[0].event_to, 1);
    assert!(
        !org.tree_chapters("s-none", r"F:\ws\demo").exists,
        "缺树 = 空态"
    );

    // 原文切片与记忆管理器材料同形（章节 b1 = 事件 0..=1 一问一答）
    let orig = org
        .tree_original("s-view", r"F:\ws\demo", "b2")
        .expect("原文可读");
    assert_eq!(orig.len(), 2);
    assert_eq!(orig[0].text, "第0轮问题");
    assert_eq!(orig[0].kind, memory_tree::MaterialKind::User);
    assert_eq!(orig[1].kind, memory_tree::MaterialKind::Assistant);
    assert!(org.tree_original("s-view", r"F:\ws\demo", "b9").is_err());
    assert!(org.tree_original("s-none", r"F:\ws\demo", "b1").is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn edit_chapter_updates_projection_and_marks_manual() {
    let root = temp_root("edit");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let session = session();
    let task = org.plan_manual(&session, "s-edit").expect("发起成功");
    org.clone().run_task(task).await;

    // 编辑标题+摘要 → 投影更新 + 手动标记 + tree.md 同步（人读层在盘）
    org.tree_edit_chapter(
        r"F:\ws\demo",
        "s-edit",
        "b2",
        Some("  人改标题  "),
        Some("人改摘要"),
    )
    .unwrap();
    let tree = tree_at(&root, org.settings.as_ref(), "s-edit");
    let b2 = tree.chapters.get("b2").unwrap();
    assert_eq!(b2.title, "人改标题");
    assert_eq!(b2.summary, "人改摘要");
    assert!(b2.title_edited_by_user);
    assert!(b2.summary_edited_by_user);
    let ws_dir = plugin_api::memlayout::project_dir_name(r"F:\ws\demo");
    assert!(
        root.join("memory/projects")
            .join(ws_dir)
            .join("tree.md")
            .exists(),
        "编辑后 tree.md 已重建"
    );

    // 校验拒绝：超上限标题 / 空+空 / 占位章节
    assert!(org
        .tree_edit_chapter(r"F:\ws\demo", "s-edit", "b2", Some(&"标".repeat(41)), None)
        .is_err());
    assert!(org
        .tree_edit_chapter(r"F:\ws\demo", "s-edit", "b2", Some("  "), Some(" "))
        .is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn delete_chapter_removes_projection_keeps_cursor() {
    let root = temp_root("delete");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let session = session();
    let task = org.plan_manual(&session, "s-del").expect("发起成功");
    org.clone().run_task(task).await;

    org.tree_delete_chapter(r"F:\ws\demo", "s-del", "b2")
        .unwrap();
    let tree = tree_at(&root, org.settings.as_ref(), "s-del");
    assert_eq!(tree.chapters.len(), 1, "b2 已摘除");
    assert!(tree.chapters.contains_key("b3"));
    assert_eq!(tree.chaptered_until, 6, "游标不动：被删区间停留已整理侧");
    // 再删不存在章节 = Err
    assert!(org
        .tree_delete_chapter(r"F:\ws\demo", "s-del", "b2")
        .is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn project_view_flattens_across_sessions() {
    let root = temp_root("projview");
    let settings = settings_with(serde_json::json!({}), &root);
    // 两个会话各整理一次（同一项目键位）
    let org = organizer(
        &root,
        settings.clone(),
        FakeClient::new(vec![Ok(good_reply()), Ok(good_reply())]),
    );
    let session = session();
    let t1 = org.plan_manual(&session, "s-p1").expect("发起成功");
    org.clone().run_task(t1).await;
    let t2 = org.plan_manual(&session, "s-p2").expect("发起成功");
    org.clone().run_task(t2).await;

    let view = org.tree_project(r"F:\ws\demo");
    assert_eq!(view.chapters.len(), 4, "两会话 × 两捆平铺");
    // 会话标注在场；编辑过的打手动标记
    assert!(view
        .chapters
        .iter()
        .all(|r| r.session_id.starts_with("s-p")));
    assert!(
        view.summaries.is_empty(),
        "good_reply 无总结段 = 项目总结链为空"
    );
    let _ = std::fs::remove_dir_all(&root);
}
