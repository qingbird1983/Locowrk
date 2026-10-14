//! 队列 Q10 拆出：**排序**（候选 → 交付的那一次比较 + 截断 + E5 保留槽）。
//!
//! 输入是已经算好信号的候选集（`signals.rs`），输出带着分数（`Scored`）——
//! 这是 Q10 相对 Q9 唯一的形状改动，理由在 `rank` 的注释里。

use kernel_agent::RecallQuery;

use super::signals::{score, Scores};
use super::Candidate;

/// 排序结果 = 候选 + 它为什么在这个位置（Q10/L4 的载荷形状）。
#[derive(Debug, Clone)]
pub(crate) struct Scored {
    pub(crate) cand: Candidate,
    pub(crate) scores: Scores,
}

/// 排序 + 截断 + E5 保留槽（稿 §3.3 末段）。纯函数，不碰 SQL——四信号的判据
/// 全在纯函数层（`signals.rs` 的算式 + 本文件的排法），所以它必须能在没有数据库
/// 的情况下被钉死。
///
/// `cands` **必须**已按 `updated_at DESC, key ASC` 排好（候选 SQL 给的就是这个
/// 序）：它同时是 ①E5 里"最近更新那条"的取法（首元素）②平票时的 tie-break。
/// tie-break 与现役 SQL 逐字同序不是巧合——**加权全为零时必须退化成现役行为**
/// （稿 §5.4 的 E4「新旧路径活在同一刻度上」），判据 `zero_weights_equal_current`
/// 钉的就是这条。
///
/// ⚠ **Q10 起返回 [`Scored`] 而不是 `Candidate`**：分数在这一层算出来就必须带出去，
/// 否则 L4 只能"再算一遍"——而再算一遍要的是同一个 `now` 与同一份词表，跨会话
/// 重放根本拿不到，给出的解释会与真发生过的那次不一致。**发生那次存回执**才是
/// 可解释的正解（与 Q1 的 `keys` 列同一条纪律）。
pub(crate) fn rank(cands: Vec<Candidate>, q: &RecallQuery, limit: usize, now: i64) -> Vec<Scored> {
    if limit == 0 || cands.is_empty() {
        return Vec::new();
    }
    // "最近更新那条"由函数自己挑，**不约定入参顺序**：依赖隐式入参序的前置条件
    // 迟早会咬人——本批第一次写测试时就按"分数序"传了候选，于是 E5 挑到的
    // newest 是错的、断言也跟着错。200 条候选再扫一遍是 O(n)，不值得为它设契约。
    let newest = cands
        .iter()
        .min_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.key.cmp(&b.key))
        })
        .expect("rank: candidates checked non-empty")
        .clone();
    let newest_key = newest.key.clone();
    // 每条只算一次分再排（原来是 sort_by 里两边各算一次，O(n log n) 次重算）。
    // 现在 `now` 固定，两种写法同值；改成先算后是因为**分数要带得出去**。
    let mut scored = cands
        .into_iter()
        .map(|c| {
            let scores = score(&c, &q.terms, q, now);
            Scored { cand: c, scores }
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| {
        b.scores
            .total
            .cmp(&a.scores.total)
            .then_with(|| b.cand.updated_at.cmp(&a.cand.updated_at))
            .then_with(|| a.cand.key.cmp(&b.cand.key))
    });
    let mut out = scored.into_iter().take(limit).collect::<Vec<_>>();
    // E5：恒保 ≥1 条"最近更新"，不因相关性排序被挤掉（稿理由：现役是"最近 5 条"，
    // 纯相关性排会把用户刚写的东西埋掉 = **行为回归**，不是优化）。
    // 实现取「顶替末位」而不是「追加一条」：追加会让交付条数越过用户设的 top_k，
    // 那是拿用户的预算换本刀的保底。顶替后不再重排 = 保留槽落在块尾，
    // 主序（分数序）不受扰动。
    //
    // ⚠ **`limit >= 2` 才做**（本刀的取舍）：保留槽花的是一个相关性名额。
    // top_k=1 时它的花费 = 100%——唯一名额给"最新那条"，相关性彻底失效，
    // 用户问"怎么部署"却只看到刚写的"今天天气不错"。那比被埋更糟，所以
    // 单条档让位给分数；稿的"恒保 ≥1"在这个档上如实不成立，注释在此。
    if limit >= 2 && !out.iter().any(|s| s.cand.key == newest_key) {
        let scores = score(&newest, &q.terms, q, now);
        if let Some(slot) = out.last_mut() {
            *slot = Scored {
                cand: newest,
                scores,
            };
        }
    }
    out
}
