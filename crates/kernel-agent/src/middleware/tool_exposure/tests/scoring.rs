//! 打分与入选（自 `tests.rs` 批 F-16 拆出，用例本体逐字未改）：always 直通不占额度 +
//! Top-N 补位 / 会话热与全局 EWMA 排序 / 会话热是布尔不是次数 / 无需裁剪也记快照。

use super::*;

#[tokio::test]
async fn always_pass_through_and_top_n_fills_quota() {
    let tools = vec![
        spec("fs_read", Exposure::Always),
        spec("search_tools", Exposure::Always),
        spec("a1", Exposure::OnDemand),
        spec("a2", Exposure::OnDemand),
        spec("a3", Exposure::OnDemand),
    ];
    let l = listener(HashMap::new(), cfg_lock(true, 3));
    let (decision, out) = run(&l, tools, Vec::new()).await;
    assert!(matches!(decision, WaterfallDecision::Replace(_)));
    // always 2 个直通不占额度，quota = 3 - 2 = 1，只补一个候选
    let names: Vec<&str> = out.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["fs_read", "search_tools", "a1"]);
    let snap = l.snapshot();
    assert_eq!(snap.total, 5);
    assert_eq!(snap.exposed, 3);
    assert_eq!(snap.always, vec!["fs_read", "search_tools"]);
    assert!(snap.saved_tokens_estimate > 0);
}

#[tokio::test]
async fn scoring_orders_session_and_global() {
    let tools = vec![
        spec("git_diff", Exposure::OnDemand),
        spec("cold", Exposure::OnDemand),
    ];
    // cold 全局很热（0.7×5=3.5 > 会话 1.0）：总分排序 cold 赢
    let ewma: HashMap<String, f64> = [("cold".into(), 5.0)].into();
    let l = listener(ewma, cfg_lock(true, 1));
    let (_, out) = run(&l, tools.clone(), messages_with_session_use("git_diff")).await;
    assert_eq!(out.tools[0].name, "cold");
    let snap = l.snapshot();
    assert_eq!(snap.global_hot, vec!["cold"]);
    assert!(snap.session_hot.is_empty());

    // cold 无全局分时，会话内用过的 git_diff 稳进
    let l2 = listener(HashMap::new(), cfg_lock(true, 1));
    let (_, out2) = run(&l2, tools, messages_with_session_use("git_diff")).await;
    assert_eq!(out2.tools[0].name, "git_diff");
    assert_eq!(l2.snapshot().session_hot, vec!["git_diff"]);
}

#[tokio::test]
async fn session_hot_is_boolean_not_count() {
    // 会话内用过 2 次与 1 次同权重（1.0），不叠加
    let mut messages = messages_with_session_use("t1");
    messages.extend(messages_with_session_use("t1"));
    let tools = vec![
        spec("t1", Exposure::OnDemand),
        spec("t2", Exposure::OnDemand),
    ];
    let l = listener(HashMap::new(), cfg_lock(true, 2));
    let (_, out) = run(&l, tools, messages).await;
    assert_eq!(out.tools.len(), 2);
    assert_eq!(l.snapshot().session_hot, vec!["t1"]);
}

#[tokio::test]
async fn no_trimming_needed_still_records_snapshot() {
    let tools = vec![spec("a", Exposure::OnDemand)];
    let l = listener(HashMap::new(), cfg_lock(true, 20));
    let (_, out) = run(&l, tools, Vec::new()).await;
    assert_eq!(out.tools.len(), 1);
    let snap = l.snapshot();
    assert!(snap.enabled);
    assert_eq!(snap.total, 1);
    assert_eq!(snap.exposed, 1);
    assert_eq!(snap.saved_tokens_estimate, 0);
    // 无会话无热度：纯补位不归组
    assert!(snap.session_hot.is_empty() && snap.global_hot.is_empty());
}
