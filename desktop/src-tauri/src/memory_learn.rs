//! 队列 Q9（ML 稿 §3.3 排序 + §3.4 强化 = 稿内 L2/L3 合一批）：注入路径的
//! **四信号现算加权排序**与**命中写回**。
//!
//! 分工：本文件只管"怎么排"与"怎么记"，**不管开关与观测**——开关在内核
//! `MemoryInjectConfig::learn`，观测仍走宿主的 `log_inject`（Q1/L0 那张表）。
//! 调用点只有一个：`memory.rs` 的 [`kernel_agent::MemorySource::recall_relevant`]
//! 覆写（全仓唯一生产读取点，形状判据盯着）。
//!
//! 四条纪律来自稿，逐条有落点：
//! - **§3.1 原则 2「落盘与计算分离」**：四个信号全部现算，落盘的只有
//!   `hit_count` / `last_hit_at` 两列（Q5/L1 建的）。`reinforce` 的 SET 列表
//!   因此是本文件的承重墙，判据钉它不许多出第三列。
//! - **§3.1 原则 3（E3 死信号剔除）**：`scope` 不进权重表——注入恒取全局层
//!   （`workspace = ''`），层标在候选集里恒定，参与排序就是摆设。
//! - **§3.1 原则 1「永不删除」**：这里没有任何 DELETE。
//! - **§3.4 不做的事**：不查 LLM 判"有没有真用上"、不按 retention 加权增量、
//!   不加独立 `weight` 列。
//!
//! ⚠ **本文件对稿的五条实测订正**（逐条在下面对应位置有注释，收口批同步进行
//! 稿内标注）：①`freshness` 的兜底取 `updated_at` 而不是稿写的 `created_at`
//! （表里从未有这一列，旧行也回填不了）；②四信号用**千分比整数**而不是浮点
//! （排序必须逐次可复现，浮点加法次序还会引入平票判定的不确定性）；③相关性
//! 的匹配面截到注入上限（不截就是给"模型看不见的字节"打分）；④候选集**有界**
//! （200 条），不是稿 §5.3 那句"全量取"；⑤写回次数 = 每会话一次而不是每轮一次
//! ——那不是本文件的决定，是内核预取缓存的形状，见 `recall_relevant` 的注释。

use rusqlite::{params, Connection};

use kernel_agent::{RecallQuery, VALUE_INJECT_MAX_CHARS};

/// 一个候选行（学习面）。与 `MemoryRow` 分开是**有意的**：管理页那张行形状
/// 是 IPC 载荷，学习列不该透给前端（Q5 起就这么定，L4 才决定要不要露出来）；
/// 顺带让"学习列只在这里被读"成为一句可以机械核对的话。
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) tags: String,
    pub(crate) updated_at: i64,
    pub(crate) hit_count: i64,
    /// `None` = 从未被召回过（Q5 的列语义）。
    pub(crate) last_hit_at: Option<i64>,
}

/// 候选集上限（订正④）。稿 §5.3 说"全量取 + 内存排序即可，不需要建索引"——
/// 前半句照做会**没有上界**：`value` 单条上限 256KiB（`memory.rs::VALUE_MAX`），
/// 库涨到几千条时一次首查就能读进几百 MB。取 200 条是把"不建索引"这个结论
/// 保住、把"全量"这个说法换成有界：
/// - top_k 的产品上限远小于 200，200 条候选足够让加权有意义（E1：过滤/打分
///   必须发生在**比交付更宽的候选集**上，否则候选池被必然出局者填满、召回饿死）；
/// - 排在 200 条之后的记忆本就 `updated_at` 最旧，四信号里两个时间信号都趋 0，
///   进不了前五——除非它 `hit_count` 很高，而那正是本刀想救的情形，届时
///   收紧的是这个常数而不是加索引（改它 = 一次决策，写在这里而不是散在 SQL 里）。
pub(crate) const SCAN_LIMIT: i64 = 200;

/// `freshness` 半衰期（天，稿 §3.3）。`1/(1+d/h)` 在 `d = h` 时正好 0.5，
/// 所以 h 就叫半衰期，不是随手取的除数。
const FRESHNESS_HALFLIFE_DAYS: i64 = 14;
/// `recency` 半衰期（天，稿 §3.3）。
const RECENCY_HALFLIFE_DAYS: i64 = 30;
const DAY_SECS: i64 = 86_400;
/// 千分比刻度（信号值域 0..=1000）。
const PERMILLE: i64 = 1000;

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

/// 一条候选的总分 = Σ 权重 × 信号（千分比 × 千分比，**不做归一除法**）。
///
/// 为什么不除权重和：同一批候选的分母相同，**排序只取决于分子**；除法只在
/// L4（Q10 的 breakdown 展示）才有意义。这也是本刀能把稿 §5.4 那句"权重和为 1"
/// 从**硬约束**降级成"缺省值恰好和为 1000"的原因——四个滑条因此可以各自独立调，
/// 不需要"改一个就把余量摊给另外三个"那种联动机制（简单问题别想复杂）。
pub(crate) fn total(cand: &Candidate, terms: &[String], q: &RecallQuery, now: i64) -> i64 {
    let w = &q.weights;
    let rel = relevance_permille(count_hits(cand, terms));
    let str_ = strength_permille(cand.hit_count);
    let fresh = freshness_permille(cand, now);
    let rec = recency_permille(cand, now);
    w.relevance as i64 * rel
        + w.strength as i64 * str_
        + w.freshness as i64 * fresh
        + w.recency as i64 * rec
}

/// 排序 + 截断 + E5 保留槽（稿 §3.3 末段）。纯函数，不碰 SQL——四信号的判据
/// 全在这一层，所以它必须能在没有数据库的情况下被钉死。
///
/// `cands` **必须**已按 `updated_at DESC, key ASC` 排好（候选 SQL 给的就是这个
/// 序）：它同时是 ①E5 里"最近更新那条"的取法（首元素）②平票时的 tie-break。
/// tie-break 与现役 SQL 逐字同序不是巧合——**加权全为零时必须退化成现役行为**
/// （稿 §5.4 的 E4「新旧路径活在同一刻度上」），判据 `zero_weights_equal_current`
/// 钉的就是这条。
pub(crate) fn rank(
    mut cands: Vec<Candidate>,
    q: &RecallQuery,
    limit: usize,
    now: i64,
) -> Vec<Candidate> {
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
    cands.sort_by(|a, b| {
        total(b, &q.terms, q, now)
            .cmp(&total(a, &q.terms, q, now))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.key.cmp(&b.key))
    });
    let mut out = cands.into_iter().take(limit).collect::<Vec<_>>();
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
    if limit >= 2 && !out.iter().any(|c| c.key == newest_key) {
        if let Some(slot) = out.last_mut() {
            *slot = newest;
        }
    }
    out
}

/// 全局层总条数（观测用，口径与 `recall_quiet` 的 `n_candidates` 一致：
/// **全量** COUNT，不是候选上限——不然 Q1 那条指标随本刀漂掉）。
pub(crate) fn count_global(conn: &Connection) -> Result<usize, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM memories WHERE workspace = ''",
        [],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n as usize)
    .map_err(|e| format!("memory learn count: {e}"))
}

/// 取候选（**有界**，见 [`SCAN_LIMIT`]）。列顺序与 `Candidate` 一一对应。
pub(crate) fn candidates(conn: &Connection) -> Result<Vec<Candidate>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT key, value, tags, updated_at, hit_count, last_hit_at FROM memories
             WHERE workspace = ''
             ORDER BY updated_at DESC, key LIMIT ?1",
        )
        .map_err(|e| format!("memory learn scan: {e}"))?;
    let rows = stmt
        .query_map(params![SCAN_LIMIT], |r| {
            Ok(Candidate {
                key: r.get(0)?,
                value: r.get(1)?,
                tags: r.get(2)?,
                updated_at: r.get(3)?,
                hit_count: r.get(4)?,
                last_hit_at: r.get(5)?,
            })
        })
        .map_err(|e| format!("memory learn scan: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("memory learn scan: {e}"))?;
    Ok(rows)
}

/// §3.4 强化回路：**交付即命中**，`hit_count += 1`、`last_hit_at = now`。
///
/// ⚠ SET 列表是承重墙：只有这两列，**不许多出第三列**——尤其不许碰
/// `updated_at`。它同时是 `recency` 信号的唯一来源和管理页的"最近编辑"含义，
/// 一旦让召回刷新它，"被读到"就把"刚编辑过"这个事实覆盖了，并且形成
/// 自我强化的正反馈闭环（稿 §6.1 担心的污染会被放大成"永远置顶"）。
///
/// 写失败**只告警不上抛**：读失败要让注入降级为空表，写失败拦不住注入——
/// 一次计数丢失不值得把聊天陪进去（同 `log_inject` 的纪律）。
pub(crate) fn reinforce(conn: &Connection, keys: &[String], now: i64) {
    for key in keys {
        if let Err(e) = conn.execute(
            "UPDATE memories SET hit_count = hit_count + 1, last_hit_at = ?1 \
             WHERE workspace = '' AND key = ?2",
            params![now, key],
        ) {
            tracing::warn!("memory reinforce `{key}`（本次命中未计数）: {e}");
        }
    }
}

// 学习列只在 `Candidate` 里被读——这句是可以在机器上核对的，因为它不是
// `MemoryRow`（管理页 IPC 行形状），全仓把 `hit_count` 当数据读写的 SQL 只有本文件的
// 两条语句（`candidates` 的 SELECT 与 `reinforce` 的 UPDATE），判据 S2 盯着。

#[cfg(test)]
mod tests;
