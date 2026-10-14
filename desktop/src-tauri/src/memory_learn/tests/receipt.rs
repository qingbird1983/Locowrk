//! 队列 Q10（L4 / ML 稿 §3.1 原则 4）：**发生那一次的回执**。
//!
//! 这一簇钉的不是"分数算得对不对"（`formulas` 钉过了），而是**算出来的分怎么带得
//! 出去**：占比的口径、回执必须是那次排序用过的同一份分（不是读完再算一遍）、
//! JSON 的形状与键名转义。落盘面（列的三档 NULL）在 `recall_log/tests/breakdown.rs`。

use super::*;
use crate::memory_learn::explain::share_permille;
use crate::memory_learn::signals::{score, PERMILLE};

/// L4 的算式面第一档：**只有一档说话**时它占满。这条看着 trivial，但它是"占比"
/// 这个说法的定义所在——占比不是信号本身，是它对**加权后总分**的作用。
#[test]
fn a_single_speaking_signal_takes_the_whole_share() {
    let c = cand("k", "docker 部署", "ops", NOW, 0, None);
    let q = q(&["docker"], [1000, 0, 0, 0], true);
    let s = score(&c, &q.terms, &q, NOW);
    assert!(s.relevance > 0 && s.total > 0);
    // 新鲜度信号本身在场（updated_at = NOW），但权重给 0 ⇒ 占比 0。
    assert_eq!(share_permille(&s, &q), [1000, 0, 0, 0]);
}

/// 四档占比相加恒 = 1000：整除的余量归最大的那一档，不然面板会出现"999%"或者
/// "333+333+333+334 之外的那种 999"，那都是没法对人解释的数。
#[test]
fn shares_always_sum_to_a_thousand() {
    let w = default_w();
    for (hits, last, age, terms) in [
        (0usize, None, 0i64, vec![]),
        (3, Some(NOW), 2, vec!["docker"]),
        (20, Some(NOW - 9 * DAY), 40, vec!["docker", "ops"]),
        (1, Some(NOW - 100 * DAY), 300, vec!["不存在的词"]),
    ] {
        let c = cand(
            "k",
            "docker 部署",
            "ops",
            NOW - age * DAY,
            hits as i64,
            last,
        );
        let qq = q(&terms, w, true);
        let s = score(&c, &qq.terms, &qq, NOW);
        let sh = share_permille(&s, &qq);
        assert_eq!(
            sh.iter().sum::<i64>(),
            PERMILLE,
            "hits={hits} age={age} terms={:?}",
            qq.terms
        );
        assert!(sh.iter().all(|v| *v >= 0), "占比不得出现负数");
    }
}

/// 四档权重全 0 ⇒ 总分 0 ⇒ 占比**无定义**。此时给均分 250 就是把"没有任何信号
/// 说话"演成"四档均衡"，那是假解释——与 `InjectStats::store_empty_ratio` 用
/// `None` 不用 `0.0` 同一条口径。
#[test]
fn an_unweighted_score_explains_nothing_instead_of_splitting_evenly() {
    let c = cand("k", "", "", NOW - 400 * DAY, 0, None);
    let q = q(&["x"], [0, 0, 0, 0], true);
    let s = score(&c, &q.terms, &q, NOW);
    assert_eq!(s.total, 0, "四档权重全 0 时总分必须是 0");
    assert_eq!(share_permille(&s, &q), [0, 0, 0, 0]);
}

/// 回执必须是**那一次排序用过的分数**，不是读完再算一遍：重算要的是同一个 `now`
/// 与同一份词表，跨会话根本拿不到，给出的解释会与真发生过的那次不一致。
/// E5 顶替进来的那条也在循环里——它的分是 `rank` 自己现算的，不是 0 占位。
#[test]
fn the_receipt_carries_the_very_totals_the_order_was_decided_by() {
    let cands = vec![
        cand("a", "docker 部署", "ops", NOW - DAY, 3, Some(NOW)),
        cand("b", "其他", "x", NOW, 0, None),
        cand("c", "docker 镜像", "ops", NOW - 2 * DAY, 0, None),
    ];
    let q = q(&["docker"], default_w(), true);
    let rows = rank(cands.clone(), &q, 2, NOW);
    assert_eq!(rows.len(), 2);
    for r in &rows {
        assert_eq!(
            r.scores,
            score(&r.cand, &q.terms, &q, NOW),
            "`{}` 的回执与它的分数不同源",
            r.cand.key
        );
    }
}

/// JSON 回执的形状判据：合法 JSON / 权重四档在前 / 每条交付一行 / 占比凑满 1000。
/// **只带键名与分数，不带记忆正文**——那是判据 S10（`MemoryEntry` 不带学习字段）
/// 在观测面的正面写法：这里连正文的字节都不抄。
#[test]
fn the_receipt_is_valid_json_carrying_keys_and_scores_only() {
    let cands = vec![
        cand("k1", "机密正文 docker", "ops", NOW, 2, Some(NOW)),
        cand("k2", "别的正文", "x", NOW - DAY, 0, None),
    ];
    let q = q(&["docker"], default_w(), true);
    let rows = rank(cands, &q, 2, NOW);
    let json = breakdown_json(&rows, &q);
    let v: serde_json::Value = serde_json::from_str(&json).expect("breakdown 必须是合法 JSON");
    let w = default_w();
    assert_eq!(v["weights"], serde_json::json!([w[0], w[1], w[2], w[3]]));
    let got = v["rows"].as_array().expect("rows 是数组");
    assert_eq!(got.len(), rows.len());
    for (r, item) in rows.iter().zip(got) {
        assert_eq!(item["key"].as_str().unwrap(), r.cand.key);
        assert_eq!(item["total"].as_i64().unwrap(), r.scores.total);
        let sum: i64 = item["share"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_i64().unwrap())
            .sum();
        assert_eq!(sum, PERMILLE);
    }
    assert!(!json.contains("机密正文"), "观测面不得把记忆正文抄一份");
    assert!(!json.contains("别的正文"));
}

/// 第一版手搓转义在这里会产出**非法 JSON**（它只处理 `"` 与 `\`，挡不住裸控制
/// 字符）。这条用例是那次自我纠正的招函：键里带引号、反斜杠、换行、响铃与制表符，
/// 写得进去也读得出来才算数。
#[test]
fn a_nasty_key_still_yields_parseable_json() {
    let nasty = "he\"llo\\wo\nrld\u{7}tab\tend";
    let q = q(&[], default_w(), true);
    let rows = rank(vec![cand(nasty, "v", "", NOW, 0, None)], &q, 1, NOW);
    let json = breakdown_json(&rows, &q);
    let v: serde_json::Value =
        serde_json::from_str(&json).expect("带控制字符的键也得产出合法 JSON");
    assert_eq!(v["rows"][0]["key"].as_str().unwrap(), nasty);
}
