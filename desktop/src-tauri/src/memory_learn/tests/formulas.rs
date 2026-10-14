//! 四信号的**算式**与**匹配面**（稿 §3.3 那五条式子 + 订正①②③各自的落点）。
//! 判错了该去改的文件 = `memory_learn/signals.rs`。

use kernel_agent::VALUE_INJECT_MAX_CHARS;

use super::*;
use crate::memory_learn::signals::{
    age_days, count_hits, freshness_permille, recency_permille, relevance_permille, score,
    strength_permille, PERMILLE,
};

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
    assert!(score(&dead, &[], &q(&[], default_w(), true), NOW).total > 0);
    // 脏值（sqlite INTEGER 被手改到极大）不得溢出成负信号：本批第一条测试
    // 就是按着这个 panic 出来的，断言留在这里当它的墓碑
    assert!((0..=PERMILLE).contains(&strength_permille(i64::MAX)));
    assert!((0..=PERMILLE).contains(&strength_permille(i64::MIN)));
}
