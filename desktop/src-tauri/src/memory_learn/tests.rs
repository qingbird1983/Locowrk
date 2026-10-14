//! 队列 Q9 学习面单测：**四信号 + tie-break + E5 保留槽 + 匹配面截断**。
//!
//! 全部打在 `rank` / `count_hits` / 各 `*_permille` 这些**纯函数层**——它们不碰
//! sqlite，所以这四条式子能不能兑现稿 §3.3 的说法，在没有数据库的情况下就能钉死。
//! SQL 面（候选 SELECT 的列、`reinforce` 的 SET 列表）由跨文件形状判据盯
//! （`tmp/q101-shape.mjs`，台账 T8），不在这里假装有覆盖。

use kernel_agent::LearnWeights;

use super::*;

fn cand(
    key: &str,
    value: &str,
    tags: &str,
    updated_at: i64,
    hits: i64,
    last_hit: Option<i64>,
) -> Candidate {
    Candidate {
        key: key.into(),
        value: value.into(),
        tags: tags.into(),
        updated_at,
        hit_count: hits,
        last_hit_at: last_hit,
    }
}

/// 词表 helper：契约里 `RecallQuery::terms` 是 `Vec<String>`（内核切词给的所有权），
/// 测试里只想写字面量，转换收在这一个函数——否则每条用例都要 `.to_string()` 一遍，
/// 而那些 `to_string` 正是"测试自己在造什么形状"的噪声源。
fn ts(terms: &[&str]) -> Vec<String> {
    terms.iter().map(|s| s.to_string()).collect()
}

/// 固定"现在"：`age_days` 的整数算术对系统时钟敏感，测试里绝不现取时间。
const NOW: i64 = 1_700_000_000;
const DAY: i64 = 86_400;

fn q(terms: &[&str], w: [u16; 4], enabled: bool) -> RecallQuery {
    RecallQuery {
        terms: ts(terms),
        weights: LearnWeights {
            enabled,
            relevance: w[0],
            strength: w[1],
            freshness: w[2],
            recency: w[3],
        },
    }
}

fn keys(out: &[Candidate]) -> Vec<String> {
    out.iter().map(|c| c.key.clone()).collect()
}

/// 缺省权重（稿 §3.3 的四档）。**测试里不写那四个数**——从内核那份真源取，
/// 将来改缺省时这里跟着动，不会出现"测试钉住旧缺省"的假绿。
fn default_w() -> [u16; 4] {
    let d = LearnWeights::default();
    [d.relevance, d.strength, d.freshness, d.recency]
}

#[test]
fn age_days_clamps_future_and_counts_whole_days() {
    assert_eq!(age_days(NOW, NOW), 0);
    assert_eq!(age_days(NOW, NOW - 2 * DAY), 2);
    // 差 23 小时仍算 0 天：整天粒度 = 同一天内两次首查给同一字节（M1）
    assert_eq!(age_days(NOW, NOW - 23 * 3600), 0);
    // 未来时间戳（时钟回拨 / 手工改表）不得把衰减顶成大于 1
    assert_eq!(age_days(NOW, NOW + 10 * DAY), 0);
}

#[test]
fn strength_saturates_and_never_reaches_one() {
    assert_eq!(strength_permille(0), 0);
    assert_eq!(strength_permille(1), 500);
    assert_eq!(strength_permille(9), 900);
    // 稿 §3.4 的霸榜担忧：1 万次与 10 万次的差距必须小到不影响排序量级
    let big_gap = strength_permille(100_000) - strength_permille(10_000);
    assert!(big_gap <= 9, "饱和后仍拉开 {big_gap} 档 = 没饱和");
    assert!(strength_permille(i64::MAX) < PERMILLE, "永不超 1");
    // 脏数据（负数列值）不得给出负信号
    assert_eq!(strength_permille(-5), 0);
}

/// 订正①的实证：`created_at` 从未存在，兜底取 `updated_at` ⇒ 未命中期两个时间
/// 信号**共线**。这不是缺陷而是登记过的形状；测试钉住它，免得将来有人把它当
/// bug"顺手修掉"改成另一个来源。
#[test]
fn freshness_falls_back_to_updated_at_when_never_hit() {
    let never = cand("a", "", "", NOW - 14 * DAY, 0, None);
    // 同一年龄：14 天半衰期那档给 500，30 天那档给 1000·30/44
    assert_eq!(freshness_permille(&never, NOW), 500);
    assert_eq!(recency_permille(&never, NOW), PERMILLE * 30 / 44);
    // 一旦有过命中就分道（freshness 看命中、recency 仍看编辑）
    let hit_now = cand("a", "", "", NOW - 14 * DAY, 3, Some(NOW));
    assert_eq!(freshness_permille(&hit_now, NOW), PERMILLE);
    assert_eq!(
        recency_permille(&hit_now, NOW),
        PERMILLE * 30 / 44,
        "recency 只认 updated_at，命中不许动它"
    );
}

#[test]
fn relevance_counts_distinct_terms_not_fields() {
    let c = cand("deploy", "use docker for deploy", "ops", NOW, 0, None);
    // 两个词、其中一个在 key 与 value 各命中一次 ⇒ 计 2 不是 3（计数单位是词）
    assert_eq!(count_hits(&c, &ts(&["deploy", "docker"])), 2);
    assert_eq!(count_hits(&c, &ts(&["k8s"])), 0);
    // tags 也算命中面（稿：key/value/tags 任一）
    assert_eq!(count_hits(&c, &ts(&["ops"])), 1);
    // 空词表 = 相关性不贡献（其余三信号仍生效），不是"全部命中"
    assert_eq!(count_hits(&c, &[]), 0);
    assert_eq!(relevance_permille(0), 0);
    assert_eq!(relevance_permille(1), 500);
}

/// 大小写：词表在核内侧已 lowercase，匹配面这边必须跟上，否则 "Use Docker"
/// 永远匹配不上 "docker"（中文无此问题，英文混排是常态）。
#[test]
fn matching_is_case_insensitive_on_the_memory_side() {
    let mixed = cand("Deploy-Steps", "Use Docker Compose", "Ops", NOW, 0, None);
    assert_eq!(count_hits(&mixed, &ts(&["docker", "deploy", "ops"])), 3);
}

/// 订正③：匹配面截到注入上限。一条记忆"因为模型根本看不见的第 600 字命中"
/// 而排到前面，是在给上下文之外的内容让路。
#[test]
fn match_face_is_clipped_to_the_injection_limit() {
    let in_face = format!("{} needle", "x".repeat(VALUE_INJECT_MAX_CHARS - 20));
    let past_face = format!("{}needle", "y".repeat(VALUE_INJECT_MAX_CHARS));
    assert_eq!(
        count_hits(&cand("a", &in_face, "", NOW, 0, None), &ts(&["needle"])),
        1
    );
    assert_eq!(
        count_hits(&cand("b", &past_face, "", NOW, 0, None), &ts(&["needle"])),
        0,
        "超出注入面的命中不得参与排序"
    );
}

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
        out.iter().any(|c| c.key == "just-written"),
        "刚写的东西被埋 = 行为回归，实得 {:?}",
        keys(&out)
    );
    // 它落在块尾（主序 = 分数序，不被保留槽扰动）
    assert_eq!(out.last().unwrap().key, "just-written");
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

/// 信号必须**同刻度**（稿 E4「新旧路径的乘子活在同一刻度上，A/B 才可比」）：
/// 任何一档给满时都不得超过 1000，否则四档相加的相对权重是假的。
#[test]
fn every_signal_stays_within_the_permille_scale() {
    let c = cand("k", "v", "t", NOW - 400 * DAY, 10_000, Some(NOW));
    assert!(strength_permille(c.hit_count) <= PERMILLE);
    assert!(freshness_permille(&c, NOW) <= PERMILLE);
    assert!(recency_permille(&c, NOW) <= PERMILLE);
    assert!(relevance_permille(64) <= PERMILLE);
    // 最老、从未命中、无相关性：两个衰减档**趋 0 但不被判 0**——稿 §1.2 的
    // "变弱但永不消失"在这里由算式给出（整数截断要到 d ≥ 30000 天 ≈ 82 年才会
    // 真的归零，那时行仍在、仍可被 tie-break 排到，登记为实现边界不是缺陷）。
    let dead = cand("k", "", "", NOW - 4000 * DAY, 0, None);
    assert_eq!(recency_permille(&dead, NOW), PERMILLE * 30 / (30 + 4000));
    assert_eq!(freshness_permille(&dead, NOW), PERMILLE * 14 / (14 + 4000));
    assert!(recency_permille(&dead, NOW) > 0);
    assert!(total(&dead, &[], &q(&[], default_w(), true), NOW) > 0);
    // 脏值（sqlite INTEGER 被手改到极大）不得溢出成负信号：本批第一条测试
    // 就是按着这个 panic 出来的，断言留在这里当它的墓碑
    assert!((0..=PERMILLE).contains(&strength_permille(i64::MAX)));
    assert!((0..=PERMILLE).contains(&strength_permille(i64::MIN)));
}
