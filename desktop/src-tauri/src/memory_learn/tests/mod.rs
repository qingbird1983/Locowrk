//! 队列 Q9–Q10 学习面单测的**共用夹具层**：候选构造、词表、固定"现在"、缺省权重。
//!
//! 全部打在 `rank` / `count_hits` / 各 `*_permille` 这些**纯函数层**——它们不碰
//! sqlite，所以这些式子能不能兑现稿 §3.3 的说法，在没有数据库的情况下就能钉死。
//! SQL 面（候选 SELECT 的列、`reinforce` 的 SET 列表）**不在这里假装有覆盖**：
//! 它由跨文件形状判据盯，判据本体与台账条目见 docs/design/ROADMAP.md 的相应批次行
//! （历批的 `tmp/q101-shape.mjs` 那种出处是 gitignore 的临时件，不该当判据的名字写进注释）。
//!
//! 队列 Q10 起分三个簇：`formulas` 钉算式，`ordering` 钉排法，`receipt` 钉回执
//! （占比 / JSON / 键名转义）。分簇的口径 = "这条用例判错了你会去改哪个文件"。

use kernel_agent::{LearnWeights, RecallQuery};

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

/// 断言用的一列 key。**Q10 起 `rank` 返回 `Scored`**（分数必须跟着排完的那次
/// 走，见 `order.rs` 的 `Scored` 注释），所以这里取 `.cand.key` 而不是
/// `.key`——测试面跟着返回形状一起改，不是把断言放宽。
fn keys(out: &[Scored]) -> Vec<String> {
    out.iter().map(|s| s.cand.key.clone()).collect()
}

/// 缺省权重（稿 §3.3 的四档）。**测试里不写那四个数**——从内核那份真源取，
/// 将来改缺省时这里跟着动，不会出现"测试钉住旧缺省"的假绿。
fn default_w() -> [u16; 4] {
    let d = LearnWeights::default();
    [d.relevance, d.strength, d.freshness, d.recency]
}

/// 四信号算式与匹配面（稿 §3.3 的五个函数，Q10 拆族时原样搬过来）。
mod formulas;
/// 排序：tie-break / 加权归零时回到现役 / E5 保留槽（Q10 拆族时原样搬过来）。
mod ordering;
/// 发生那一次的回扮：占比、JSON 形状与键名转义（队列 Q10 / L4）。
mod receipt;
