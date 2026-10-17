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
    assert!(matches!(decision, WaterfallDecision::Continue));
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

// ---- 批 F-23（审计 M-24c）：逐项 JSON 长度重构 + `select` 返回下标的等价证明 ----
//
// 病根两条：① 每请求把整个工具目录（含 schema）整体序列化**两次**只为求一个长度
// （`spec_tokens(全目录)` 与 `spec_tokens(保留集)`）；② `select` 把保留下来的
// `ToolSpec` 逐个克隆，调用方再整份赋回 `req.tools`（目录没被裁剪时也照克隆）。
// 改法 = ① 逐项 `to_writer` 进计数 sink 求字节数、保留集长度由下标子集求和；
// ② `select` 只返回下标、调用方按下标**移动**。下面两段各拿 HEAD 原文当参考实现。

/// **被删原文**：HEAD 版 `spec_tokens`（整体序列化后 `/4`）。
fn ref_spec_tokens_old(specs: &[ToolSpec]) -> u64 {
    serde_json::to_string(specs).map_or(0, |s| s.len() as u64) / 4
}

/// 长度参差的目录：名字长短不一、描述含中文与 emoji（`to_string` 不转义非 ASCII ⇒
/// **字节数 ≠ 字符数**，谁把某一侧写成 `.chars().count()` 这里就红）、schema 嵌套
/// 深浅各不同（空对象 / 带 required / 带数值约束）。
fn varied_catalog() -> Vec<ToolSpec> {
    vec![
        spec("a", Exposure::Always),
        ToolSpec {
            name: "fs_read".into(),
            description: "读取文件（支持中文与 emoji 🚀）".into(),
            schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "绝对路径" } },
                "required": ["path"]
            }),
            side_effect: SideEffect::Pure,
            exposure: Exposure::Always,
        },
        ToolSpec {
            name: "x".into(),
            description: String::new(),
            schema: json!({}),
            side_effect: SideEffect::Irreversible,
            exposure: Exposure::OnDemand,
        },
        ToolSpec {
            name: "memory_search_with_a_long_name".into(),
            description: "跨会话检索".repeat(7),
            schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100 }
                }
            }),
            side_effect: SideEffect::Pure,
            exposure: Exposure::OnDemand,
        },
        spec("shell", Exposure::OnDemand),
    ]
}

#[test]
fn json_len_matches_whole_vector_serialization() {
    let catalog = varied_catalog();
    // 逐项长度 = 单项整体序列化的字节数
    for s in &catalog {
        assert_eq!(
            spec_json_len(s),
            serde_json::to_string(s).unwrap().len() as u64,
            "spec_json_len({}) 与整体序列化字节数不等",
            s.name
        );
    }
    let lens: Vec<u64> = catalog.iter().map(spec_json_len).collect();
    // 阳性对照：目录里真有非 ASCII（否则"字节 vs 字符"这一档是空的）
    assert!(
        lens.iter().sum::<u64>()
            > catalog
                .iter()
                .map(|s| s.description.chars().count() as u64)
                .sum::<u64>(),
        "目录没有非 ASCII 内容 ⇒ 字节/字符之分测不出来"
    );

    let subsets: Vec<Vec<usize>> = vec![
        vec![],
        vec![0],
        vec![4],
        vec![0, 1],
        vec![1, 3, 4],
        (0..catalog.len()).collect(),
        // 逆序：求和与次序无关，钉住"重构不偷偷排序"
        (0..catalog.len()).rev().collect(),
    ];
    assert_eq!(array_json_len(&[]), 2, "空数组 `[]` = 2 字节");
    for idx in &subsets {
        let specs: Vec<ToolSpec> = idx.iter().map(|&i| catalog[i].clone()).collect();
        let kept_lens: Vec<u64> = idx.iter().map(|&i| lens[i]).collect();
        // 数组长度重构 = 整体序列化的字节数（逐字节等长，不只是 token 数相等）
        assert_eq!(
            array_json_len(&kept_lens),
            serde_json::to_string(&specs).unwrap().len() as u64,
            "子集 {idx:?} 的数组字节数与整体序列化不等"
        );
        // 旧口径 = 整体序列化后 /4；新口径 = 数组字节数 /4（取整在求差之前分别做）
        assert_eq!(
            tokens_of(array_json_len(&kept_lens)),
            ref_spec_tokens_old(&specs),
            "子集 {idx:?} 的 token 估算与旧实现不等"
        );
        // 快照里那个 `saved_tokens_estimate` 的差值口径也逐格相等
        let new_saved =
            tokens_of(array_json_len(&lens)).saturating_sub(tokens_of(array_json_len(&kept_lens)));
        let old_saved = ref_spec_tokens_old(&catalog).saturating_sub(ref_spec_tokens_old(&specs));
        assert_eq!(
            new_saved, old_saved,
            "子集 {idx:?} 的 saved 差值与旧实现不等"
        );
    }
    // 阳性对照：这些数字不是"两边都 0"的假绿
    assert!(ref_spec_tokens_old(&catalog) > 0);
    assert!(
        tokens_of(array_json_len(&lens)) > tokens_of(array_json_len(&[lens[0]])),
        "全目录必须比单项长（否则 saved 恒 0）"
    );
}

/// **被删原文**：HEAD 版 `Selection`（`kept` 是克隆出来的 `Vec<ToolSpec>`）。
#[derive(Debug)]
struct RefSelection {
    kept: Vec<ToolSpec>,
    always: Vec<String>,
    session_hot: Vec<String>,
    global_hot: Vec<String>,
    retrieved: Vec<String>,
}

/// **被删原文**：HEAD 版 `select`（逐字抄，只把返回类型换成 [`RefSelection`]）。
fn ref_select_old(
    catalog: &[ToolSpec],
    session: &HashMap<String, u32>,
    ewma: &HashMap<String, f64>,
    retrieval: &HashMap<String, f64>,
    max_tools: usize,
) -> RefSelection {
    let (always_specs, candidates): (Vec<&ToolSpec>, Vec<&ToolSpec>) = catalog
        .iter()
        .partition(|s| matches!(s.exposure, Exposure::Always));
    let quota = max_tools.saturating_sub(always_specs.len());

    let mut scored: Vec<(&ToolSpec, f64)> = candidates
        .into_iter()
        .map(|s| {
            let hit = session.contains_key(&s.name);
            let score = SESSION_WEIGHT * u32::from(hit) as f64
                + GLOBAL_WEIGHT * ewma.get(&s.name).copied().unwrap_or(0.0)
                + RETRIEVAL_WEIGHT * retrieval.get(&s.name).copied().unwrap_or(0.0);
            (s, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut kept: Vec<ToolSpec> = always_specs.iter().map(|s| (*s).clone()).collect();
    let always: Vec<String> = kept.iter().map(|s| s.name.clone()).collect();
    let mut session_hot = Vec::new();
    let mut global_hot = Vec::new();
    let mut retrieved = Vec::new();
    for (spec, _score) in scored.into_iter().take(quota) {
        if session.contains_key(&spec.name) {
            session_hot.push(spec.name.clone());
        } else if ewma.get(&spec.name).copied().unwrap_or(0.0) > 0.0 {
            global_hot.push(spec.name.clone());
        } else if retrieval.get(&spec.name).copied().unwrap_or(0.0) > 0.0 {
            retrieved.push(spec.name.clone());
        }
        kept.push(spec.clone());
    }
    RefSelection {
        kept,
        always,
        session_hot,
        global_hot,
        retrieved,
    }
}

#[test]
fn select_by_index_equals_old_cloning_select() {
    let catalog = varied_catalog();
    let sessions: Vec<HashMap<String, u32>> = vec![
        HashMap::new(),
        [("a".into(), 1u32)].into(),
        [("fs_read".into(), 3u32), ("shell".into(), 1u32)].into(),
    ];
    let ewmas: Vec<HashMap<String, f64>> = vec![
        HashMap::new(),
        [("x".into(), 5.0)].into(),
        [
            ("shell".into(), 0.1),
            ("memory_search_with_a_long_name".into(), 9.0),
        ]
        .into(),
    ];
    let retrievals: Vec<HashMap<String, f64>> = vec![HashMap::new(), [("x".into(), 0.8)].into()];
    // 同分档：全部候选分数相同 ⇒ 只能靠 stable sort 保住目录原序
    let tie_catalog: Vec<ToolSpec> = (0..6)
        .map(|i| spec(&format!("t{i}"), Exposure::OnDemand))
        .collect();

    let mut cases = 0usize;
    let (mut saw_trim, mut saw_session, mut saw_global, mut saw_retrieved) =
        (false, false, false, false);
    for (cat, label) in [(&catalog, "参差目录"), (&tie_catalog, "同分目录")] {
        for session in &sessions {
            for ewma in &ewmas {
                for retrieval in &retrievals {
                    for max_tools in [0usize, 1, 2, 3, 5, 20] {
                        let ctx = format!(
                            "{label} session={session:?} ewma={ewma:?} retrieval={retrieval:?} max_tools={max_tools}"
                        );
                        let want = ref_select_old(cat, session, ewma, retrieval, max_tools);
                        let got = select(cat, session, ewma, retrieval, max_tools);
                        // 下标 → spec：与旧实现的克隆序列**逐格同值同序**
                        let got_kept: Vec<ToolSpec> =
                            got.kept_idx.iter().map(|&i| cat[i].clone()).collect();
                        assert_eq!(got_kept, want.kept, "kept 分岔：{ctx}");
                        assert_eq!(got.always, want.always, "always 分岔：{ctx}");
                        assert_eq!(got.session_hot, want.session_hot, "session_hot 分岔：{ctx}");
                        assert_eq!(got.global_hot, want.global_hot, "global_hot 分岔：{ctx}");
                        assert_eq!(got.retrieved, want.retrieved, "retrieved 分岔：{ctx}");
                        // `reorder` 的移动版依赖两条不变量：下标唯一 + 不越界
                        let mut sorted = got.kept_idx.clone();
                        sorted.sort_unstable();
                        sorted.dedup();
                        assert_eq!(
                            sorted.len(),
                            got.kept_idx.len(),
                            "kept_idx 有重复下标：{ctx}"
                        );
                        assert!(
                            got.kept_idx.iter().all(|&i| i < cat.len()),
                            "kept_idx 越界：{ctx}"
                        );
                        // 移动版重排 = 旧的"按下标克隆"（含 always 直通与同分保序）
                        let mut pool = cat.clone();
                        assert_eq!(
                            reorder(&mut pool, &got.kept_idx),
                            want.kept,
                            "reorder 与旧克隆序列不等：{ctx}"
                        );
                        assert!(
                            pool.is_empty(),
                            "移动版必须把源目录掏空（元素已移走）：{ctx}"
                        );
                        saw_trim |= want.kept.len() < cat.len();
                        saw_session |= !want.session_hot.is_empty();
                        saw_global |= !want.global_hot.is_empty();
                        saw_retrieved |= !want.retrieved.is_empty();
                        cases += 1;
                    }
                }
            }
        }
    }
    // 阳性对照：网格真的覆盖到"有裁剪"与三种归因，不是全空转
    assert!(cases >= 200, "网格太小 = {cases}");
    assert!(saw_trim, "网格里一次裁剪都没发生 ⇒ kept 等值没有证明力");
    assert!(saw_session, "网格没覆盖 session_hot 归因");
    assert!(saw_global, "网格没覆盖 global_hot 归因");
    assert!(saw_retrieved, "网格没覆盖 retrieved 归因");
}

#[test]
fn reorder_falls_back_to_clone_on_duplicate_index() {
    // 钉死表指向重名工具时会出现重复下标：退化为逐个克隆（不是 panic、也不是去重）
    let catalog = varied_catalog();
    let idx = vec![0usize, 0, 2, 2, 2];
    let mut pool = catalog.clone();
    let got = reorder(&mut pool, &idx);
    assert_eq!(
        got,
        idx.iter().map(|&i| catalog[i].clone()).collect::<Vec<_>>(),
        "重复下标必须逐个产出同值副本"
    );
    assert_eq!(got.len(), idx.len(), "重复下标不是去重");
    // 退化路径在 `mem::take` **之前**返回 ⇒ 源目录原封不动（与移动版的语义差异写死，
    // 防将来把两条路径合并时顺手改掉）
    assert_eq!(pool, catalog, "克隆退化路径不该掏空源目录");
}
