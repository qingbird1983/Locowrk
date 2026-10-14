//! 队列 Q9（ML 稿 §3.3 排序 + §3.4 强化 = 稿内 L2/L3 合一批）：注入路径的
//! **四信号现算加权排序**与**命中写回**。
//!
//! 分工：本模块只管"怎么排"与"怎么记"，**不管开关与观测**——开关在内核
//! `MemoryInjectConfig::learn`，观测仍走宿主的 `log_inject`（Q1/L0 那张表）。
//! 调用点只有一个：`memory.rs` 的 [`kernel_agent::MemorySource::recall_relevant`]
//! 覆写（全仓唯一生产读取点，形状判据盯着）。
//!
//! 四条纪律来自稿，逐条有落点：
//! - **§3.1 原则 2「落盘与计算分离」**：四个信号全部现算，落盘的只有
//!   `hit_count` / `last_hit_at` 两列（Q5/L1 建的）。`reinforce` 的 SET 列表
//!   因此是本模块的承重墙，判据钉它不许多出第三列。
//! - **§3.1 原则 3（E3 死信号剔除）**：`scope` 不进权重表——注入恒取全局层
//!   （`workspace = ''`），层标在候选集里恒定，参与排序就是摆设。
//! - **§3.1 原则 1「永不删除」**：这里没有任何 DELETE。
//! - **§3.4 不做的事**：不查 LLM 判"有没有真用上"、不按 retention 加权增量、
//!   不加独立 `weight` 列。
//!
//! ⚠ **本模块对稿的五条实测订正**（逐条在下面对应位置有注释，收口批同步进行
//! 稿内标注）：①`freshness` 的兜底取 `updated_at` 而不是稿写的 `created_at`
//! （表里从未有这一列，旧行也回填不了）；②四信号用**千分比整数**而不是浮点
//! （排序必须逐次可复现，浮点加法次序还会引入平票判定的不确定性）；③相关性
//! 的匹配面截到注入上限（不截就是给"模型看不见的字节"打分）；④候选集**有界**
//! （200 条），不是稿 §5.3 那句"全量取"；⑤写回次数 = 每会话一次而不是每轮一次
//! ——那不是本模块的决定，是内核预取缓存的形状，见 `recall_relevant` 的注释。
//!
//! **队列 Q10 拆出三个子文件**（本文件因 explain 面涨到 420 行，越过 400 建议线；
//! 三问自检里第 ③ 问"修改它是否经常牵动不相关的部分"答"是"——加解释层时不该牵动
//! SQL 面）：`signals.rs` = 四信号的算式，`order.rs` = 排序 + E5 保留槽，
//! `explain.rs` = 把分数编成给人看的回执。正文逐字节搬移，唯一一处字面改动 =
//! `PERMILLE` 从私有升 `pub(crate)`（子簇 `tests.rs` 不是 `signals` 的孩子，
//! 祖先私有项对它不可见；抄第二份刻度常数 = 同一读数两个出处，本仓立过纪律的那条）。
//! SQL 面（取候选 / 计数 / 写回）留在下面本文件里：它是"怎么记"的那一半，
//! 与 `Candidate` 的行形状同源，拆开会切开一条判据（S2 数的是全仓把 `hit_count`
//! 当数据读写的 SQL 语句条数）。

use rusqlite::{params, Connection};

mod explain;
mod order;
mod signals;

// 只重新导出**生产读取点用到的**那几个名字（`memory.rs` 的 `recall_relevant`）。
// 四信号的算式本体（`signals` 那九个）不在这里铺平：给它们加 `pub(crate) use` 而
// 生产不引用，只会让 lib 构建报 unused import，然后被人用 `#[allow]` 按掉——
// 真正用它们的是同目录的 `tests.rs`，那里按名字直取（`use super::signals::*`），
// "谁在读这四条式子"因此在代码里可见而不是藏在一条转发里。
pub(crate) use explain::breakdown_json;
pub(crate) use order::{rank, Scored};

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
