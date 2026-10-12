//! 检索与历史两把工具：search_titles（语义缺席降级关键词 / 轮总结正文参与打分）与
//! project_history（跨会话时间线 / 最近 20 条钳制 + 无总结章节落 other_chapters）。

use super::*;

#[tokio::test]
async fn search_titles_falls_back_to_keyword_without_embedding() {
    let root = temp_root("titles");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let session = session();
    let task = org.plan_manual(&session, "s-st").expect("发起成功");
    org.clone().run_task(task).await;

    let tool = crate::organize::SearchTitlesTool::new(
        memory_tree::TreeStore::new(&root),
        resolver(),
        None, // embedding 缺席 = 关键词降级
        root.join("sessions"),
    );
    use plugin_api::{CallCtx, Tool as _};
    let mut ctx = CallCtx::new();
    ctx.session_id = Some("s-st".into());
    // 语义缺席 → 关键词 contains 命中且 match_mode 如实
    let out = tool
        .call(&ctx, serde_json::json!({"query": "话题一"}))
        .await
        .expect("可调用");
    assert_eq!(out["match_mode"], "keyword");
    assert_eq!(out["hits"].as_array().unwrap().len(), 1);
    assert_eq!(out["hits"][0]["title"], "话题一");
    let _ = std::fs::remove_dir_all(&root);
}

/// v2.142 索引扩容：查询词只出现在**轮总结正文**（标题/摘要都不含）也
/// 命中——总结是浓缩叙事，宽查询的语义主体。
#[tokio::test]
async fn search_titles_scores_round_summary_text_not_just_titles() {
    let root = temp_root("titles-summary");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(
        &root,
        settings,
        FakeClient::new(vec![Ok(good_reply_with_summary())]),
    );
    let task = org.plan_manual(&session(), "s-st2").expect("发起成功");
    org.clone().run_task(task).await;

    let tool = crate::organize::SearchTitlesTool::new(
        memory_tree::TreeStore::new(&root),
        resolver(),
        None,
        root.join("sessions"),
    );
    use plugin_api::{CallCtx, Tool as _};
    let mut ctx = CallCtx::new();
    ctx.session_id = Some("s-st2".into());
    let out = tool
        .call(&ctx, serde_json::json!({"query": "共享 runtime"}))
        .await
        .expect("可调用");
    assert_eq!(out["match_mode"], "keyword");
    let hits = out["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "总结正文命中: {out}");
    assert!(
        hits[0]["session_title"].is_string(),
        "hits 带 session_title 字段"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// v2.142 时间线（Cross-Session Recall Design v1 §2.3）：项目跨会话
/// 时间正序 ×（总结 + 所含章节标题）=「之前做了什么」的第一答案面。
#[tokio::test]
async fn project_history_returns_timeline_grouped_by_summary() {
    let root = temp_root("history");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(
        &root,
        settings,
        FakeClient::new(vec![
            Ok(good_reply_with_summary()),
            Ok(good_reply_with_summary()),
        ]),
    );
    let task = org.plan_manual(&session(), "s-h1").expect("发起成功");
    org.clone().run_task(task).await;
    let task = org.plan_manual(&session(), "s-h2").expect("发起成功");
    org.clone().run_task(task).await;

    let tool = crate::organize::ProjectHistoryTool::new(
        memory_tree::TreeStore::new(&root),
        resolver(),
        root.join("sessions"),
    );
    use plugin_api::{CallCtx, Tool as _};
    let mut ctx = CallCtx::new();
    ctx.session_id = Some("s-h2".into());
    let out = tool
        .call(&ctx, serde_json::json!({}))
        .await
        .expect("可调用");
    assert_eq!(out["truncated"], false);
    let sessions = out["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2, "两会话都在时间线");
    assert_eq!(sessions[0]["session_id"], "s-h1", "时间正序");
    let sums = sessions[0]["summaries"].as_array().unwrap();
    assert_eq!(sums.len(), 1, "每会话一轮总结");
    assert!(sums[0]["text"].as_str().unwrap().contains("共享 runtime"));
    let covered = sums[0]["chapters"].as_array().unwrap();
    assert_eq!(covered.len(), 2, "该轮两章都归入总结（区间包含）");
    assert_eq!(
        covered[0]["id"], "b2",
        "章节带 id 供 expand 寻址（占位捆占 b1，管道章节自 b2 起）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Q2（MR 稿 §4.3）：`memory_project_history` 的档位必须是 **Always**，
/// `side_effect` 必须是 **Pure**。
///
/// 这两条是"提档"这件事唯一的机器保证，而且不是审美：留在 OnDemand 时，新会话
/// 零会话内用量 + embedding 未配置 ⇒ 语义相似度退化 ⇒ 被 Top-20 截断**挤出目录**，
/// 模型根本看不见这个工具（MR 稿 §0-2 的实测成因，也是那次"查一下你之前的记忆"
/// 答错的直接原因）。`side_effect` 同批钉住是因为**计划模式围栏按 side_effect
/// 放行、与 exposure 无关**——不钉住这条，后来人会以为"提档要顺手动围栏"。
#[test]
fn project_history_is_always_exposed_and_still_pure() {
    use plugin_api::{Exposure, SideEffect, Tool as _};
    let root = temp_root("ph-exposure");
    let tool = crate::organize::ProjectHistoryTool::new(
        memory_tree::TreeStore::new(&root),
        resolver(),
        root.join("sessions"),
    );
    let spec = tool.spec();
    assert_eq!(spec.exposure, Exposure::Always, "档位必须已提到 Always");
    assert_eq!(
        spec.side_effect,
        SideEffect::Pure,
        "读侧零副作用（围栏据此放行）"
    );
    assert_eq!(spec.name, "memory_project_history", "工具身份不变");
    assert!(
        spec.description.contains("memory_expand"),
        "描述仍指路下钻（提档不许顺手改文案语义）"
    );
}

/// 时间线钳制：>20 会话取最近 20 + truncated 如实 + 指路 search_titles；
/// 无总结覆盖的章节落 other_chapters（v2.141 前存量会话形态）。
#[tokio::test]
async fn project_history_caps_to_recent_sessions_and_keeps_uncovered() {
    let root = temp_root("history-cap");
    let ws = r"F:\ws\demo";
    let store = memory_tree::TreeStore::new(&root);
    for i in 0..21 {
        let sid = format!("t{i:02}");
        let mut tree = memory_tree::MemoryTree::new(ws, &sid);
        let d = vec![memory_tree::sanitize_draft(&format!("章节{i}"), "摘要", (0, 1)).unwrap()];
        let b = tree.build_chapters(&d, "recorder", 1);
        // t20 不带总结 = 章节无覆盖 → other_chapters（存量形态）
        let summary = (i != 20).then(|| format!("第{i}轮总结"));
        tree.apply_organized(
            b,
            1,
            memory_tree::OrganizeRecord {
                ts: i as i64,
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
            summary,
        );
        store.save(&tree).unwrap();
    }
    let tool = crate::organize::ProjectHistoryTool::new(store, resolver(), root.join("sessions"));
    use plugin_api::{CallCtx, Tool as _};
    let mut ctx = CallCtx::new();
    ctx.session_id = Some("t20".into());
    let out = tool
        .call(&ctx, serde_json::json!({}))
        .await
        .expect("可调用");
    assert_eq!(out["total_sessions"], 21);
    assert_eq!(out["truncated"], true);
    assert!(out["note"]
        .as_str()
        .unwrap()
        .contains("memory_search_titles"));
    let sessions = out["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 20, "钳最近 20");
    assert_eq!(sessions[0]["session_id"], "t01", "最早的 t00 被钳掉");
    // 无总结会话：章节落 other_chapters（标题在场，expand 可下钻）
    let last = &sessions[sessions.len() - 1];
    assert_eq!(last["session_id"], "t20");
    assert_eq!(last["summaries"].as_array().unwrap().len(), 0);
    let other = last["other_chapters"].as_array().unwrap();
    assert_eq!(other.len(), 1);
    assert_eq!(other[0]["id"], "b1");
    let _ = std::fs::remove_dir_all(&root);
}
