//! 落账形状判据（回执行写成什么样子，夹具在父簇 `mod.rs`）。

use serde_json::json;

use super::{call_through, obs, read_hit, two_layer_matches, FakeRetrieval};

// ---------------------------------------------------------------- 落账形状

/// 回执行写的是**工具名**当 `mode`、`n_candidates` 恒 0（工具结果里没有候选总数这个量，
/// 编一个数就是造假观测），`session_key` 与注入档同串。
#[tokio::test]
async fn receipt_row_shape() {
    let (sink, lg) = obs("shape");
    call_through(
        FakeRetrieval::ok("memory_search", two_layer_matches()).arc(),
        sink.clone(),
        Some("sess-1"),
    )
    .await
    .unwrap();
    let rows = lg.recent(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].mode, "memory_search");
    assert_eq!(rows[0].session_key, "sess-1");
    assert_eq!(rows[0].returned, 2);
    assert_eq!(
        rows[0].keys,
        vec!["user.stack".to_string(), "proj.lang".to_string()]
    );
    assert_eq!(
        rows[0].n_candidates, 0,
        "检索档没有候选总数出处，恒 0（读取面别把它当「库里没货」）"
    );

    // 无会话 = 空串而不是 NULL：与注入档同款（部分唯一索引里 NULL ≠ NULL）。
    call_through(
        FakeRetrieval::ok("memory_read", read_hit("user.stack", "global")).arc(),
        sink,
        None,
    )
    .await
    .unwrap();
    let rows = lg.recent(10).unwrap();
    assert_eq!(rows[0].session_key, "");
    assert_eq!(rows[0].mode, "memory_read");
}

/// **键位与注入档同源**：同一个会话，两档写出的 `scope_id` 必须是同一个串——它出自
/// 解析器 + `workspace::scope_key`，**不是**结果 JSON（实测命中行只有层标
/// `scope: "global"|"project"`，没有 workspace 路径；把层标塞进这列就是同列两义）。
/// 未归属 / 无会话 ⇒ 空串，含义是「该会话未归属」，与注入档的空串同一个解释。
#[tokio::test]
async fn scope_id_is_the_same_keyside_string_as_the_injection_side() {
    let (sink, lg) = obs("scope");
    let hit = read_hit("a", "project");

    // 归属会话：键位 = 解析器给的项目串（与 `scope_key(Some(..))` 逐字相等）
    call_through(
        FakeRetrieval::ok("memory_read", hit.clone()).arc(),
        sink.clone(),
        Some("s1"),
    )
    .await
    .unwrap();
    assert_eq!(
        lg.recent(5).unwrap()[0].scope_id,
        crate::workspace::scope_key(Some("F:/ws/a")),
        "两档的键位串必须出自同一个归一函数"
    );

    // 未归属会话：空串（层标 "project" 不许渗进来）
    call_through(
        FakeRetrieval::ok("memory_read", hit).arc(),
        sink,
        Some("unattributed-session"),
    )
    .await
    .unwrap();
    assert_eq!(
        lg.recent(5).unwrap()[0].scope_id,
        "",
        "未归属 = 空串；结果里的层标不是键位"
    );
}

/// `memory_read` 的 `found: false` 是**成功返回零条** ⇒ 照记一行 `returned = 0`。
/// 这一行是"模型问了但没有这条"的唯一出处，漏记等于把缺口藏起来。
#[tokio::test]
async fn memory_read_miss_is_a_zero_row_not_a_missing_row() {
    let (sink, lg) = obs("miss");
    call_through(
        FakeRetrieval::ok("memory_read", json!({"found": false, "key": "a"})).arc(),
        sink,
        Some("s1"),
    )
    .await
    .unwrap();
    let rows = lg.recent(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].returned, 0);
    assert!(rows[0].keys.is_empty());
}

/// 结果形状与假设脱钩 ⇒ **不记**（而不是记一条 0），但调用本身照原样成功返回。
/// 插件侧改了字段名时这里会安静地停止出账，`tracing::warn` 是唯一信号——
/// 所以"不记"这一半必须与"仍返回原 payload"一起判，否则漂移就成静默的。
#[tokio::test]
async fn shape_drift_writes_no_row_and_does_not_break_the_call() {
    let (sink, lg) = obs("drift");
    let payload = json!({"hits": [{"key": "a"}]}); // 缺 `matches`
    let got = call_through(
        FakeRetrieval::ok("memory_search", payload.clone()).arc(),
        sink,
        Some("s1"),
    )
    .await
    .unwrap();
    assert_eq!(got, payload);
    assert!(lg.recent(10).unwrap().is_empty());
}

/// `memory_project_history` 的交付是**会话数**，`keys` 恒空（它给的是标题不是记忆 key）。
/// 这条是对 L0 稿 §2.3(c) 分子的实测订正：把它的"标题"塞进 `keys` 就会得到一个
/// 永远为 0 的假命中率。
#[tokio::test]
async fn project_history_counts_sessions_and_carries_no_keys() {
    let (sink, lg) = obs("history");
    call_through(
        FakeRetrieval::ok(
            "memory_project_history",
            json!({
                "workspace": "F:/ws/a",
                "total_sessions": 2,
                "truncated": false,
                "sessions": [
                    {"session_id": "s1", "session_title": "批 F", "summaries": [], "other_chapters": []},
                    {"session_id": "s2", "session_title": "批 G", "summaries": [], "other_chapters": []}
                ]
            }),
        )
        .arc(),
        sink,
        Some("s1"),
    )
    .await
    .unwrap();
    let rows = lg.recent(10).unwrap();
    assert_eq!(rows[0].mode, "memory_project_history");
    assert_eq!(
        rows[0].returned, 2,
        "交付 = 会话条数（它没有「记忆条数」这个口径）"
    );
    assert!(rows[0].keys.is_empty(), "标题不是 memory key");
}

/// 去重口径的反面：注入档按键位去重（Q1a 立的 `ux_mil_inject`），**检索档不去重**——
/// 同一个工具连着查三次就是三次真实发生，去重会把"查了三遍"读成"查了一遍"。
#[tokio::test]
async fn receipts_are_not_deduplicated_unlike_injections() {
    let (sink, lg) = obs("dedup");
    for _ in 0..3 {
        call_through(
            FakeRetrieval::ok(
                "memory_search",
                json!({"query": "q", "matches": [read_hit("a", "global")]}),
            )
            .arc(),
            sink.clone(),
            Some("s1"),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        lg.recent(10).unwrap().len(),
        3,
        "三次成功检索就该有三行（部分唯一索引只覆盖 mode='inject'）"
    );
    // 同键位对照：注入档写五次只剩一行。
    for _ in 0..5 {
        lg.note_inject(
            "s1",
            "F:/ws/a",
            1,
            1,
            &["a".to_string()],
            std::time::Duration::from_micros(1),
            None,
        )
        .unwrap();
    }
    let injects: Vec<_> = lg
        .recent(20)
        .unwrap()
        .into_iter()
        .filter(|r| r.mode == "inject")
        .collect();
    assert_eq!(injects.len(), 1, "同一键位的重复召回不重复出账");
}
