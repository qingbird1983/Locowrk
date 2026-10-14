//! 队列 Q10 拆出：**把分数编成给人看的回执**（ML 稿 §3.1 原则 4 / §5.1 L4）。
//!
//! 分工边界：本文件不做任何"排第几"的决定（那是 `order.rs`），只把已经排完的
//! 结果里的四信号贡献摊成占比并写成 JSON。它产出的字节只去观测面
//! （`memory_inject_log.breakdown`），**永远不去 system**——`MemoryEntry`
//! 不带学习字段这条判据（S10）就是给这里划的界。

use kernel_agent::RecallQuery;

use super::order::Scored;
use super::signals::Scores;

/// 每一档对总分的**贡献占比**（千分比）。这就是稿 §3.1 原则 4 要的那一句
/// "这条为什么排在这"，也是全稿唯一一处需要除法的地方：
/// `share_i = w_i·s_i·1000 / total`，四档相加 = 1000（整除余量归最大的一档，
/// 免得界面出现"三档 333 + 一档 334"这种说不清的 1‰）。
///
/// `total == 0` 时给全 0：此时"占比"无定义（四档都没贡献），而不是均分 250——
/// 均分会把"没有任何信号说话"演成"四档均衡"，那是假解释。
pub(crate) fn share_permille(s: &Scores, q: &RecallQuery) -> [i64; 4] {
    let w = &q.weights;
    if s.total <= 0 {
        return [0; 4];
    }
    let raw = [
        w.relevance as i64 * s.relevance,
        w.strength as i64 * s.strength,
        w.freshness as i64 * s.freshness,
        w.recency as i64 * s.recency,
    ];
    let mut out = raw.map(|v| v * 1000 / s.total);
    let rest = 1000 - out.iter().sum::<i64>();
    if rest > 0 {
        let top = out
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| **v)
            .map(|(i, _)| i)
            .unwrap_or(0);
        out[top] += rest;
    }
    out
}

/// 把交付的分数快照编成观测 JSON（列 `memory_inject_log.breakdown`）。
///
/// 形状 = `{"weights":[…4…],"rows":[{"key","relevance","strength","freshness",
/// "recency","total","share"⋯},…]}`。权重一起落，是因为占比读出来必须有分母才能
/// 核对（面板要能显示"那一次四档滑条各在哪"）；两个数组都是**按位**的，顺序 =
/// `LearnWeights` 的字段序（relevance, strength, freshness, recency），读取面
/// `inject_view.rs` 用同一个顺序，不在这儿再声明一份名字。
/// 键序由 serde 的 map 决定（字典序），读取面按键取、不依赖顺序。
///
/// ⚠ **用 `serde_json::json!` 而不是手搓拼串**：这条是本批自己抓到的返工——第一版
/// 为了"键名一眼可见"自己拼串并配了个 `escape_json_key`，而那个转义函数只处理
/// `"` 与 `\`，键里一个裸控制字符（从聊天里复制来的键、手工改表改出来的键）就
/// 会产出**非法 JSON**：那比"没有解释"更坏，读取面会把整行判成坏数据、连带这条
/// 观测通路在这一点上失明。`serde_json` 本来就在依赖里（`Cargo.toml:58`），
/// 该转的字符它全管，这种面一行都不该自己写。
pub(crate) fn breakdown_json(rows: &[Scored], q: &RecallQuery) -> String {
    let w = &q.weights;
    serde_json::json!({
        "weights": [w.relevance, w.strength, w.freshness, w.recency],
        "rows": rows
            .iter()
            .map(|r| {
                let sh = share_permille(&r.scores, q);
                serde_json::json!({
                    "key": r.cand.key,
                    "relevance": r.scores.relevance,
                    "strength": r.scores.strength,
                    "freshness": r.scores.freshness,
                    "recency": r.scores.recency,
                    "total": r.scores.total,
                    "share": sh,
                })
            })
            .collect::<Vec<_>>(),
    })
    .to_string()
}
