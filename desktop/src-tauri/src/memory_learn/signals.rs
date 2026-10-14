//! 队列 Q10 拆出：四信号的**算式**本体（ML 稿 §3.3 那张信号表 + §3.1 原则 4 的
//! "每个信号都要可解释"）。
//!
//! 本文件只管"一个候选值多少分"，不管"这些分怎么排成一份交付"（`order.rs`），
//! 也不管"怎么把分讲给人听"（`explain.rs`），更不碰 SQL——四信号的判据因此
//! 全部落在无数据库的纯函数层（`memory_learn/tests/`）。
//!
//! ⚠ **千分比整数而不是浮点**（对稿订正②）：排序必须逐次可复现，浮点加法次序
//! 还会引入平票判定的不确定性。四信号同刻度（稿 E4「新旧路径活在同一刻度上，
//! A/B 才可比」），所以本文件里所有 `*_permille` 的值域都是 `0..=1000`。

use kernel_agent::{RecallQuery, VALUE_INJECT_MAX_CHARS};

use super::Candidate;

/// `freshness` 半衰期（天，稿 §3.3）。`1/(1+d/h)` 在 `d = h` 时正好 0.5，
/// 所以 h 就叫半衰期，不是随手取的除数。
const FRESHNESS_HALFLIFE_DAYS: i64 = 14;
/// `recency` 半衰期（天，稿 §3.3）。
const RECENCY_HALFLIFE_DAYS: i64 = 30;
const DAY_SECS: i64 = 86_400;
/// 千分比刻度（信号值域 0..=1000）。
pub(crate) const PERMILLE: i64 = 1000;

/// 距今多少个**整天**（未来的时间戳按 0 天算——时钟回拨/手工改表不能让一条
/// 记忆比"刚刚"还新，负数会把 `1/(1+d/h)` 顶成大于 1，破坏四信号同刻度）。
pub(crate) fn age_days(now: i64, at: i64) -> i64 {
    (now - at).max(0) / DAY_SECS
}

/// `strength = hit_count / (1 + hit_count)`：饱和曲线，收敛快且永不超 1
/// （稿 §3.4：线性累加会让"被检索 100 次的冷门记忆"永久霸榜）。
///
/// ⚠ 算式取恒等变形 `1000 − ⌈1000/(h+1)⌉` 而不是字面的 `1000·h/(h+1)`：本批第一条
/// 测试就把后者按出了 **multiply with overflow**（`hit_count` 是 sqlite INTEGER，
/// 手改一次就能到 `i64::MAX`，debug 构建当场 panic、release 构建 wrap 成**负信号**
/// ——那比不钳更糟，因为它会把一条"被用过很多次"的记忆压到所有候选之下）。
/// 先钳到 1e6 再算：曲线在那个点之后与 1000 的差已小于 1 档，钳位不改排序语义。
pub(crate) fn strength_permille(hit_count: i64) -> i64 {
    let m = hit_count.clamp(0, 1_000_000) + 1;
    // 手写向上取整除法，不用 `div_ceil`：本仓工具链（rustc 1.98.1）在这里报
    // **E0658 use of unstable library feature `int_roundings`**，第一次撞上就换成
    // 算术形式——为一档饱和曲线开 feature gate 不值当。
    PERMILLE - (PERMILLE + m - 1) / m
}

/// 平滑衰减 `1/(1 + d/h)` 的整数形 = `1000·h / (h + d)`（同一条式的分子分母
/// 同乘 h，没有除法精度损失）。`d` 用 `age_days` 的整天粒度——四信号里两个
/// 都是它，同粒度才能保证同一天内的两次首查给出同一字节（M1）。
fn decay_permille(halflife_days: i64, d: i64) -> i64 {
    PERMILLE * halflife_days / (halflife_days + d)
}

/// `freshness`：最近是否被命中过。
///
/// ⚠ 订正①：稿写的是 `last_hit_at ?? created_at`，而 `memories` 表**从未有
/// `created_at` 这一列**（Q5 只加 `hit_count` / `last_hit_at`），旧行也回填不了
/// （建行时间没有任何留存）。兜底取 `updated_at`——语义是"从没被用过就先按
/// 最近更新算热度"，与 `recency` 在未命中期**共线**（同一个年龄、不同半衰期）。
/// 这个共线是良性的：一条从没被召回过的记忆，"新鲜"本来就是它唯一的可用信号；
/// 一旦有过命中，两者分道（14 天的那只看命中，30 天的那只看编辑）。
/// 真正不能接受的是让 `freshness` 恒等于 0（那等于稿的信号表里少一项 = 违反 E3
/// 的反面——留一个永不响的仪表）或新建一列（迁移不可逆，且存量数据全为 NULL）。
pub(crate) fn freshness_permille(cand: &Candidate, now: i64) -> i64 {
    let at = cand.last_hit_at.unwrap_or(cand.updated_at);
    decay_permille(FRESHNESS_HALFLIFE_DAYS, age_days(now, at))
}

/// `recency`：时间新鲜度（现役 `updated_at DESC` 的平滑版）。
pub(crate) fn recency_permille(cand: &Candidate, now: i64) -> i64 {
    decay_permille(RECENCY_HALFLIFE_DAYS, age_days(now, cand.updated_at))
}

/// `relevance = 命中数 / (命中数 + 1)`，无命中 = 0（稿 §3.3）。
///
/// ⚠ 订正③：**匹配面截到注入上限**。内核 `build_block` 把单条 value 截到
/// [`VALUE_INJECT_MAX_CHARS`] 字符才进 system，所以一条记忆"因为第 8000 个字符
/// 命中而排到第一"是在给模型根本看不到的内容让路。复用内核那颗常量而不是
/// 在这里再写一个 500——同一属性两个出处会漂（本仓历批立的纪律）。
pub(crate) fn relevance_permille(hits: usize) -> i64 {
    let h = (hits as i64).min(1_000_000);
    PERMILLE * h / (h + 1)
}

/// 一条候选对这次请求命中了几个词（稿：命中 key/value/tags 任一即计 1，
/// 计数单位是**词**不是字段）。
///
/// 与 `memory_search` 同纪律：**不建索引、无词典**——这里只是内存里的
/// `contains`，词表由内核从最近三条用户消息切出（含 CJK 2 字滑窗，否则中文
/// 恒不命中）。大小写：词表在核内侧已 lowercase，这里把匹配面也 lowercase；
/// 汉字不受影响。
pub(crate) fn count_hits(cand: &Candidate, terms: &[String]) -> usize {
    if terms.is_empty() {
        return 0;
    }
    let key = cand.key.to_lowercase();
    let tags = cand.tags.to_lowercase();
    let value: String = cand
        .value
        .chars()
        .take(VALUE_INJECT_MAX_CHARS)
        .collect::<String>()
        .to_lowercase();
    terms
        .iter()
        .filter(|t| {
            key.contains(t.as_str()) || value.contains(t.as_str()) || tags.contains(t.as_str())
        })
        .count()
}

/// 一条候选在某次排序里的**四信号读数**（全是千分比）。
///
/// ⚠ `total` 是 Σ 权重×信号（千分比×千分比，**不做归一除法**）：同一批候选分母
/// 相同，排序只取决于分子，所以除法在这层不需要——它只在 [`share_permille`]
/// （= 稿 §3.1 原则 4 的展示面）里才有意义。把"权重和必须为 1000"钉成硬约束，
/// 就要为界面造"改一个滑条摊平其余三个"的联动机制（本仓 Q9 批次已把这降级成
/// 缺省巧合，这里保持同一个口径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Scores {
    /// 相关性档（0..=1000 千分比，命中词数派生）。
    pub(crate) relevance: i64,
    pub(crate) strength: i64,
    pub(crate) freshness: i64,
    pub(crate) recency: i64,
    /// Σ 权重×信号（**未归一**，量级到 10^6；排序用的就是它）。
    pub(crate) total: i64,
}

/// 一条候选的**四信号快照**（§3.1 原则 4「每个信号都要可解释」的正身）。
///
/// ⚠ 这一层的产物**只走观测旁路**：既不进内核 `MemoryEntry`（那等于把机器状态
/// 摆进 system，还花 token——Q9 的判据 S10 至今钉着），也不进管理页的 IPC 行形状
/// `MemoryRow`。它落盘到 `memory_inject_log.breakdown`，由只读面板回放。
/// 稿 §3.1 原则 2「参与排序的量不落盘」**没有被这一条破**：落盘的是"那次排序算完
/// 之后的读数"（一张回执，像 Q1 的 keys 列），不是下一次排序的输入——下一次排序
/// 仍然从 `hit_count`/`last_hit_at`/`updated_at` 现算，读不到这张回执。
pub(crate) fn score(cand: &Candidate, terms: &[String], q: &RecallQuery, now: i64) -> Scores {
    let w = &q.weights;
    let rel = relevance_permille(count_hits(cand, terms));
    let str_ = strength_permille(cand.hit_count);
    let fresh = freshness_permille(cand, now);
    let rec = recency_permille(cand, now);
    Scores {
        relevance: rel,
        strength: str_,
        freshness: fresh,
        recency: rec,
        total: w.relevance as i64 * rel
            + w.strength as i64 * str_
            + w.freshness as i64 * fresh
            + w.recency as i64 * rec,
    }
}
