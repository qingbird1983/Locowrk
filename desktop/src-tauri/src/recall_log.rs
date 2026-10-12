//! Q1a（后审计队列首刀 · 《Memory Learning Mechanism》§2.3 L0 的 (a) + (c) 一半）：
//! **注入侧召回可观测性**。
//!
//! **这块治什么**：`MemoryStore::recall()` 长期零观测——无计数、无「注入了哪些 key」、
//! 无「库里到底有多少可选」。后果有实测证据：《Memory Redesign v1》§0 那份诊断（模型
//! 绕开有货的记忆树去读 handoff.md）是靠**人工翻 982 KB audit.jsonl** 推断出来的。
//! 本模块把注入侧每一轮交付变成机器可读的一行，供 Q2/Q3（注入面与写入面）做
//! before/after 对照，并作为 Q9（L2 改排序）的地基。
//!
//! **Q1a / Q1b 的分界（为什么切开）**：稿子的 (b)「检索回执」要挂在 `runner` 的
//! ToolResult 投影上，而那里唯一顺手可用的载体是 `UsageRecorder`——把召回观测挂进统计
//! 写入器，`usage.rs` 就同时做两件事（三问自检第②问答「否」）；另一条路是给 `run_turn`
//! 再加一个参数，而它有**十处调用点**（含 7 处测试）为一行日志字段陪绑。两条都不该和
//! 埋点本体混在同一刀里 ⇒ (b) 与 `injection_yield` 指标单独成批 = 队列 **Q1b**。
//! 表结构已为此留余地：`mode` 列现在写死 `'inject'`，Q1b 只加值、不加迁移。
//!
//! **为什么是独立文件而不是堆进 `usage.rs`**：召回观测是新职责，仓库纪律 = 新职责先建
//! 新文件、禁止堆进已有文件；且 L0 稿 §3.2 明写「命中次数与工具热度必须分开——一个是
//! 『这条记忆有用』、一个是『这个工具常用』，混在一起会互相污染」。本模块因此**自持连接**，
//! 只借用同一个 `<数据根>/usage.sqlite3` 文件另建一张表：满足稿子 §2.3(e)「不新建库、
//! 与工具热度同生命周期」的意图，代码上与 `UsageDb` 零交集。
//!
//! ⚠ **锚点漂移已登记**：稿子 §2.3(e) 给的落点 `crates/kernel-agent/src/middleware/usage.rs`
//! **在本仓库不存在**，`usage.sqlite3` 的真身是 [`crate::usage`]（由 `state/boot.rs` 开库）。
//! 见《Post-Audit Roadmap v1》§5。
//!
//! **三条不变量**（动了就是造假观测）：
//! 1. **零行为变更**——只读召回的产物，不参与召回决策；`recall()` 的 SQL 与交付结果必须
//!    与改前逐字段一致（判据 `recall_rows_are_identical_with_and_without_instrumentation`）。
//! 2. **观测不许拖垮被观测者**——写失败只 `warn`，不上抛、不 panic；库打不开 = 静默不记
//!    （与 `usage.rs` 头注「打不开只丢统计」同纪律）。
//! 3. **按键位去重**（`ux_mil_inject`）——`MemoryInject` 的会话预取缓存（`PREFETCH_CAP`）
//!    在**有 session_id 时**保证每会话一次 `recall()`；但**无 session_id 的调用每 step
//!    都会走到 recall**（`memory.rs` 里「每 step 都 recall，别刷日志」那句注释就是这个
//!    场景）。不去重 = 日志按 step 膨胀、空块率被同一会话的重复行主导。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

/// 表 + 两个索引，每次开连接重跑（`IF NOT EXISTS` 幂等）——本模块因此不需要假设启动
/// 顺序，也不给 `usage.rs` 的建表函数加一个字。
///
/// `mode` 列现在只有 `'inject'`；`session_key` 用空串表示无会话（**不用 NULL**：部分唯一
/// 索引里 NULL ≠ NULL，用 NULL 会让无会话的注入每 step 插一条，去重直接失效）。
const DDL: &str = "
CREATE TABLE IF NOT EXISTS memory_inject_log (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    ts           INTEGER NOT NULL,
    mode         TEXT NOT NULL,
    session_key  TEXT NOT NULL,
    scope_id     TEXT NOT NULL,
    n_candidates INTEGER NOT NULL,
    returned     INTEGER NOT NULL,
    keys         TEXT NOT NULL,
    elapsed_us   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mil_mode_ts ON memory_inject_log(mode, ts);
CREATE UNIQUE INDEX IF NOT EXISTS ux_mil_inject ON memory_inject_log(session_key, scope_id) WHERE mode = 'inject';";

/// 表内一行（[`RecallLog::recent`] 的形状）。
///
/// ⚠ 生产读取点 = 队列 Q10（L4 管理页），本批只有测试消费——登记为已知边界而不是删掉：
/// 埋点写了却读不出来，Q9 改排序时照样两眼一抹黑。豁免写法与本仓库 `MemoryStore::write`
/// （「本方法当前只服务测试播种」）同款。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecallRow {
    pub mode: String,
    pub session_key: String,
    pub scope_id: String,
    pub n_candidates: i64,
    pub returned: i64,
    pub keys: Vec<String>,
}

/// 派生指标（L0 稿 §2.3(c)：只从表里算，不额外埋点）。
///
/// 本批只有空块率。`injection_yield`（注入过的 key 里有多少被检索回来）需要 Q1b 的
/// 回执当分子——**现在摆一个恒为 None 的口就是「摆了个不能改变读数的通道」**
/// （L0 稿 §3.1 原则 3），故随 Q1b 一起加。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct InjectStats {
    /// 注入行数（= 参与统计的会话键位数）。
    pub injections: usize,
    /// 其中交付为空（`returned = 0`）的行数。
    pub empty_injections: usize,
    /// `store_empty_ratio` = 空块占比；零注入行时是 **None 而不是 0.0**——0.0 会被读成
    /// 「注入了但都不空」，与「一次都没注入」是两个不同的结论。
    pub store_empty_ratio: Option<f64>,
}

/// 召回观测的落库句柄：持路径，按调用开连接、用完即弃（与 `MemoryStore` 同款形状）。
#[derive(Debug, Clone)]
pub(crate) struct RecallLog {
    path: PathBuf,
}

impl RecallLog {
    /// 指向 `<数据根>/usage.sqlite3`。**这里不碰库**——建表放在每次 open，于是
    /// 「装好了但一次召回都没发生」不产生任何磁盘副作用。
    pub(crate) fn at(path: impl AsRef<Path>) -> Arc<Self> {
        Arc::new(RecallLog {
            path: path.as_ref().to_path_buf(),
        })
    }

    fn open_conn(&self) -> Result<Connection, String> {
        let conn = Connection::open(&self.path).map_err(|e| format!("recall log open: {e}"))?;
        // 与 `tool_usage` 的 5s 批量刷盘**同文件不同表**：两个写者会撞锁，
        // busy_timeout 是唯一防线（`MemoryStore::open_conn` 同款 3s）。
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(|e| format!("recall log busy_timeout: {e}"))?;
        conn.execute_batch(DDL)
            .map_err(|e| format!("recall log schema: {e}"))?;
        Ok(conn)
    }

    /// 注入档埋点（`MemoryStore::recall_quiet` 的**成功路径**调用）。
    ///
    /// 打不开库时不许走到这里：那种情况下连 `n_candidates` 都取不到，记一行 0 会把
    /// 「库不可用」冒充成「库是空的」= 指标造假（判据
    /// `unavailable_memory_store_does_not_write_a_row`）。
    pub(crate) fn note_inject(
        &self,
        session_key: &str,
        scope_id: &str,
        n_candidates: usize,
        returned: usize,
        keys: &[String],
        elapsed: Duration,
    ) -> Result<(), String> {
        let conn = self.open_conn()?;
        // INSERT OR IGNORE 的冲突源只有 `ux_mil_inject` 这一个部分唯一索引（其余列全
        // NOT NULL 且都给了值）——同键位的重复召回即在此被吞掉。
        conn.execute(
            "INSERT OR IGNORE INTO memory_inject_log
                 (ts, mode, session_key, scope_id, n_candidates, returned, keys, elapsed_us)
             VALUES (?1, 'inject', ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                now_secs(),
                session_key,
                scope_id,
                n_candidates as i64,
                returned as i64,
                keys_json(keys),
                // u128 → i64 钳制：真实时长远不到上限，但类型转换不许静默截断。
                elapsed.as_micros().min(i64::MAX as u128) as i64,
            ],
        )
        .map_err(|e| format!("recall log insert: {e}"))?;
        Ok(())
    }

    /// 空块率（L0 稿 §2.3(c) 的 `store_empty_ratio`）。读取点 = Q10，本批仅测试消费。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn stats(&self) -> Result<InjectStats, String> {
        let conn = self.open_conn()?;
        let (injections, empty_injections): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(returned = 0), 0) FROM memory_inject_log WHERE mode = 'inject'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| format!("recall log stats: {e}"))?;
        Ok(InjectStats {
            injections: injections as usize,
            empty_injections: empty_injections as usize,
            store_empty_ratio: (injections > 0)
                .then(|| empty_injections as f64 / injections as f64),
        })
    }

    /// 最近 N 行（倒序）。测试断言用；生产读取点 = Q10（L4 管理页）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn recent(&self, limit: i64) -> Result<Vec<RecallRow>, String> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT mode, session_key, scope_id, n_candidates, returned, keys
                 FROM memory_inject_log ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| format!("recall log recent: {e}"))?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(RecallRow {
                    mode: r.get(0)?,
                    session_key: r.get(1)?,
                    scope_id: r.get(2)?,
                    n_candidates: r.get(3)?,
                    returned: r.get(4)?,
                    keys: parse_keys(&r.get::<_, String>(5)?),
                })
            })
            .map_err(|e| format!("recall log recent rows: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }
}

fn keys_json(keys: &[String]) -> String {
    serde_json::to_string(keys).unwrap_or_else(|_| "[]".to_string())
}

/// 落库文本 → key 列表。解析失败按空表处理（观测面读不回脏数据，也不因此中断）。
fn parse_keys(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
