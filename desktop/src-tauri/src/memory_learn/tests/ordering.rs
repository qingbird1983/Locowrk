//! **排法**：加权归零时逐字节回到现役、相关性/强度能顶过时间、E5 保留槽的取舍。
//! 判错了该去改的文件 = `memory_learn/order.rs`（`rank`）。

use super::*;

/// **等价性的算术面**：权重全 0 ⇒ 四信号都不贡献 ⇒ 排序完全由 tie-break 决定，
/// 而 tie-break 就是现役 SQL 的 `updated_at DESC, key`。这条与 `learnEnabled=false`
/// 走 `Self::recall` 本体那条（`memory/tests.rs`）合起来，才是"回到现役"的两端。
#[test]
fn zero_weights_degrade_to_current_ordering() {
    let cands = vec![
        cand("newest", "", "", NOW, 0, None),
        cand("mid", "hit", "", NOW - DAY, 9, Some(NOW)),
        cand("oldest", "hit", "hit", NOW - 90 * DAY, 999, Some(NOW)),
    ];
    let out = rank(cands, &q(&["hit"], [0, 0, 0, 0], true), 2, NOW);
    assert_eq!(keys(&out), ["newest", "mid"]);
    // 同 updated_at 时按 key 升序（现役 ORDER BY 的第二键就是它）
    let same = vec![
        cand("b", "", "", NOW, 0, None),
        cand("a", "", "", NOW, 0, None),
    ];
    let out = rank(same, &q(&[], [0, 0, 0, 0], true), 5, NOW);
    assert_eq!(keys(&out), ["a", "b"]);
}

#[test]
fn relevance_outranks_a_newer_but_irrelevant_entry() {
    let cands = vec![
        cand("fresh-irrelevant", "今天天气好", "", NOW, 0, None),
        cand(
            "stale-relevant",
            "部署用 docker compose，先 build 再 up",
            "deploy",
            NOW - 60 * DAY,
            0,
            None,
        ),
    ];
    // 只有相关性档贡献（另三档归零），"部署"命中旧那条
    let out = rank(cands.clone(), &q(&["部署"], [1000, 0, 0, 0], true), 2, NOW);
    assert_eq!(keys(&out)[0], "stale-relevant");
    // 反过来：相关性档给 0、新鲜度档主导时，新的那条回到第一位
    let out = rank(cands, &q(&["部署"], [0, 0, 0, 1000], true), 2, NOW);
    assert_eq!(keys(&out)[0], "fresh-irrelevant");
}

#[test]
fn reinforcement_history_lifts_a_memory_above_its_peers() {
    // 三条同龄同内容，只差 hit_count ⇒ 被反复召回的那条应排前面
    let cands = vec![
        cand("cold", "a", "", NOW - 5 * DAY, 0, None),
        cand("hot", "a", "", NOW - 5 * DAY, 20, Some(NOW - DAY)),
        cand("warm", "a", "", NOW - 5 * DAY, 3, Some(NOW - DAY)),
    ];
    let out = rank(cands, &q(&[], default_w(), true), 3, NOW);
    assert_eq!(keys(&out), ["hot", "warm", "cold"]);
}

/// E5（稿 §3.3 末段）：恒保 ≥1 条"最近更新"，且**顶替末位而不是追加**——
/// 追加会让交付条数越过用户设的 top_k，那是拿用户的预算换本刀的保底。
#[test]
fn e5_reserve_swaps_the_last_slot_instead_of_exceeding_top_k() {
    let cands = vec![
        cand("just-written", "新写的偏好", "", NOW, 0, None),
        cand("rel-1", "docker 部署", "ops", NOW - 3 * DAY, 9, Some(NOW)),
        cand("rel-2", "docker 镜像", "ops", NOW - 4 * DAY, 9, Some(NOW)),
    ];
    // 相关性 + 强度 + 热度三档拉满、新鲜度归零 ⇒ "just-written" 分数最低
    let out = rank(cands, &q(&["docker"], [1000, 1000, 1000, 0], true), 2, NOW);
    assert_eq!(out.len(), 2, "保留槽不得把交付条数顶过 top_k");
    assert!(
        out.iter().any(|s| s.cand.key == "just-written"),
        "刚写的东西被埋 = 行为回归，实得 {:?}",
        keys(&out)
    );
    // 它落在块尾（主序 = 分数序，不被保留槽扰动）
    assert_eq!(out.last().unwrap().cand.key, "just-written");
}

#[test]
fn reserve_is_a_noop_when_newest_already_ranked_or_limit_is_zero() {
    let cands = vec![
        cand("top", "docker", "", NOW, 9, Some(NOW)),
        cand("tail", "other", "", NOW - 3 * DAY, 0, None),
    ];
    // 最新那条本就排第一 ⇒ 不触发顶替（条数与内容都不该动）
    let out = rank(cands.clone(), &q(&["docker"], default_w(), true), 1, NOW);
    assert_eq!(keys(&out), ["top"]);
    // limit 0（内核 top_k==0 时根本不会走到这里，这里只验函数自己的边界）
    assert!(rank(cands.clone(), &q(&[], default_w(), true), 0, NOW).is_empty());
    // 候选不足时 newest 必然在场 ⇒ 同样不该顶替出别的条
    let out = rank(cands.clone(), &q(&[], default_w(), true), 5, NOW);
    assert_eq!(out.len(), 2);
}

/// E5 的取舍面（`rank` 里 `limit >= 2` 那个条件的存在理由）：保留槽花的是一个
/// 相关性名额，top_k=1 时花费 = 100%，所以单条档让位给分数。这条用例判的就是
/// "最新那条分数最低 + limit 1"时**它没被塞进来**——稿的"恒保 ≥1"在这一档如实
/// 不成立，测试把它钉成写下来的取舍而不是漏掉的分支。
#[test]
fn reserve_yields_to_the_score_when_top_k_is_one() {
    let cands = vec![
        cand(
            "relevant-old",
            "docker 部署",
            "ops",
            NOW - 30 * DAY,
            0,
            None,
        ),
        cand("just-written", "今天天气不错", "", NOW, 0, None),
    ];
    // 相关性拉满、两个时间档归零 ⇒ "just-written"（newest）分数为 0
    let out = rank(
        cands.clone(),
        &q(&["docker"], [1000, 0, 0, 0], true),
        1,
        NOW,
    );
    assert_eq!(
        keys(&out),
        ["relevant-old"],
        "唯一名额给最新那条 = 相关性失效"
    );
    // 同一份候选给到 limit 2，保留槽才有地方站（第二条 = 最新那条）
    let out = rank(cands, &q(&["docker"], [1000, 0, 0, 0], true), 2, NOW);
    assert_eq!(keys(&out), ["relevant-old", "just-written"]);
}

/// 权重不再要求"和为 1000"（稿 §5.4 的权重表是展示口径）：同一批候选的相对序
/// 只取决于 Σ 权重×信号，整体缩放不改序。这条测试是那句话的机器形式。
#[test]
fn scaling_all_weights_together_does_not_change_the_order() {
    let cands = vec![
        cand("a", "docker", "", NOW - 2 * DAY, 4, Some(NOW - DAY)),
        cand("b", "other", "", NOW - 20 * DAY, 0, None),
    ];
    let one = rank(
        cands.clone(),
        &q(&["docker"], [400, 250, 200, 150], true),
        2,
        NOW,
    );
    let doubled = rank(cands, &q(&["docker"], [800, 500, 400, 300], true), 2, NOW);
    assert_eq!(keys(&one), keys(&doubled));
}
