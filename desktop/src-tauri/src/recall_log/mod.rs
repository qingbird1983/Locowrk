//! Q1a（后审计队列首刀 · 《Memory Learning Mechanism》§2.3 L0 的 (a) + (c) 一半）：
//! **注入侧召回可观测性**。
//!
//! **这块治什么**：`MemoryStore::recall()` 长期零观测——无计数、无「注入了哪些 key」、
//! 无「库里到底有多少可选」。后果有实测证据：《Memory Redesign v1》§0 那份诊断（模型
//! 绕开有货的记忆树去读 handoff.md）是靠**人工翻 982 KB audit.jsonl** 推断出来的。
//! 本模块把注入侧每一轮交付变成机器可读的一行，供 Q2/Q3（注入面与写入面）做
//! before/after 对照，并作为 Q9（L2 改排序）的地基。
//!
//! **L0 的两刀怎么分（Q1a / Q1b）**：(a) 注入侧埋点 + (c) 的 `store_empty_ratio` = Q1a
//! （v2.183），(b) 检索回执 + (c) 的 `injection_yield` = Q1b（本文件现在的读取面）。分刀的
//! 真实理由不是工作量而是**载体**：回执要挂在「拿得到工具名 + 结果本体」的地方，可选的三处
//! 只剩一处能用——① 洋葱中间件：`Next::tool_name/tool_spec/args` 全是 `pub(crate)`
//! （`crates/kernel-agent/src/middleware/mod.rs:97-112`），宿主自己实现的中间件**看不见是
//! 哪个工具**，此路要把内核 API 面放宽，不属于观测批；② `runner::record_stats` 的 ToolResult
//! 扫描：要么给 `run_turn` 再加一个参数（**十处调用点**为一行日志陪绑），要么挤进
//! `UsageRecorder`（`usage.rs` 就同时做两件事，三问自检第②问答「否」）；③ 剩下的
//! [`crate::tool_source`] 解析出口 = 唯一「一次接线覆盖内置与插件两条通路、且不动任何签名」
//! 的位置 ⇒ 回执采集件挂在 `resolve` 的返回值上（见 [`crate::recall_receipt`]）。
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
//! 见已归档的施工队列 `docs/history/post-audit-roadmap-v1.md` §5。
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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

mod schema;
// 库形状（建表 DDL / `mode` 取值 / 加列迁移）在 `schema.rs`：本文件其余部分只管
// "怎么写、怎么读"。队列 Q10 拆出，正文逐字节搬移，不改一条 SQL 的字面量。
/// 注入通道的 `mode` 取值：**生产读取点 `inject_view` 要用它筛行**，所以这里
/// `pub(crate) use` 出去而不是私用。字符串只有 `schema.rs` 一个出处（部分唯一索引
/// 的谓词必须是字面量，其余一律绑这个常量）。
pub(crate) use schema::MODE_INJECT;
use schema::{ensure_breakdown_column, DDL};

// `DDL` 与 `MODE_INJECT` 的正身在 `schema.rs`（上面 `use` 进来）——同一张表的形状两处各
// 存一份才是真事故，所以这里只引用、不复制。

// 回执行的 `mode` = 工具名，`keys` = 该次结果里返回的 memory key（写入面见
// `RecallLog::note_receipt`，读取面见 `RecallLog::stats`）。
//
// ⚠ **`memory_project_history` 的结果里没有 key**（它返回会话标题与章节标题，实测见
// `crate::organize::project_history`）——L0 稿 §2.3(c) 把 `injection_yield` 的分子写成
// 「被 `memory_search` / `memory_project_history` 检索到的 key 数」，后半句在真实数据结构上
// **不成立**（两个 key 空间不相交，硬凑只会得到一个恒 0 的假指标）。本实现因此按「非注入档
// 行的 `keys` 并集」取分子：不带 key 的工具落一行空表，对交集零贡献 = 不污染读数，而它的
// `returned` 仍然是有用观测（模型到底走不走时间线）。见已归档的施工队列 `docs/history/post-audit-roadmap-v1.md` §5。

/// `insert_row` 的语句形状：注入档按键位去重（同键位重复召回不重复出账），检索档一次成功
/// 调用一行（一次检索就是一次真实发生，去重会把"模型查了三遍"读成"查了一遍"）。
#[derive(Debug, Clone, Copy)]
enum Dedup {
    ByKeyside,
    EveryCall,
}

/// 落一行的载荷：注入档与检索档**共用同一个形状**，差别只在 `mode` 的取值与去重口径。
/// 写成结构体而不是九个位置参数（`insert_row` 因此 3 个参数）= clippy
/// `too_many_arguments` 的正面解法，也让"列 ↔ 值"的对应只在一处出现。
struct Row<'a> {
    mode: &'a str,
    session_key: &'a str,
    scope_id: &'a str,
    n_candidates: usize,
    returned: usize,
    keys: &'a [String],
    elapsed: Duration,
    /// Q10（L4）：**这次排序算出来的四信号回执**（JSON）。`None` = 这一档没有可解释
    /// 的分数——现役注入（学习档关掉）与检索回执都是 `None`，落 NULL 而不是空串：
    /// 空串会被读成"有回执但回执是空的"，与"这一路根本没有分数"是两个结论
    /// （与 `store_empty_ratio` 用 `None` 不用 `0.0` 同一条口径）。
    breakdown: Option<&'a str>,
}

/// 表内一行（[`RecallLog::recent`] 的形状）。
///
/// ⚠ 生产读取点已经在 **Q10** 落位（`memory_inject_view` → 管理页的只读注入解释区块），
/// 所以这里的 `allow(dead_code)` 豁免摘掉了——留着一句"还没有读取点"的注释而它已经接线，
/// 就是第六道闸立时要抓的那种漂移。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecallRow {
    /// 落盘时刻（epoch 秒）。Q10 的面板要说"这是哪一次的注入"，没有它就只能说
    /// "最近一次"——那等于把观测表里现成的一个数藏起来。
    pub ts: i64,
    pub mode: String,
    pub session_key: String,
    pub scope_id: String,
    pub n_candidates: i64,
    pub returned: i64,
    pub keys: Vec<String>,
    /// Q10（L4）四信号回执的**原始 JSON**（`None` = 这一行没有回执：现役注入档与检索档）。
    /// 这里刻意不解析成结构：解析由读取层 `crate::inject_view` 做，本层只保证"读回来的是
    /// 库里那一串"，坏 JSON 在读取层降级为"无法解释"而不是把整条观测通路带崩。
    pub breakdown: Option<String>,
}

/// 派生指标（L0 稿 §2.3(c)：只从表里算，不额外埋点）。
///
/// 两个比率都按**键位集合**算而不是按次数：同一 key 被查三遍不会把命中率抬高
/// （稿 §6.1「学习信号可能被『检索即命中』污染」的第一道防线就是把计数换成集合）。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct InjectStats {
    /// 注入行数（= 参与统计的会话键位数）。
    pub injections: usize,
    /// 其中交付为空（`returned = 0`）的行数。
    pub empty_injections: usize,
    /// `store_empty_ratio` = 空块占比；零注入行时是 **None 而不是 0.0**——0.0 会被读成
    /// 「注入了但都不空」，与「一次都没注入」是两个不同的结论。
    pub store_empty_ratio: Option<f64>,
    /// 注入过的 **key 去重全集**大小 = `injection_yield` 的分母（不是注入行数：同一 key 在
    /// 两个会话键位各注入一次，分母仍是 1，否则"注入次数"会把比率推到 1 以上）。
    pub injected_keys: usize,
    /// 注入过的 key 里被检索回来的个数 = 分子（只算交集，见 `note_receipt` 的口径说明）。
    pub yielded_keys: usize,
    /// `injection_yield` = `yielded_keys / injected_keys`；一个 key 都没注入过时是
    /// **None**——0.0 会被读成「注入了但模型没用上」，与「根本没注入」是两个结论（与
    /// `store_empty_ratio` 同款口径）。
    pub injection_yield: Option<f64>,
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
        // 队列 Q10（L4）：`CREATE TABLE IF NOT EXISTS` 对**已存在的表不补列**，所以存量库
        // 的 `breakdown` 必须显式迁。形状逐字照 `memory.rs::ensure_learning_schema`（探针 →
        // `BEGIN IMMEDIATE` → 等锁后二次探针 → 补列 → COMMIT/ROLLBACK）——同一把尺在本仓
        // 只该有一套写法，两处各发明一份迟早会漂。
        ensure_breakdown_column(&conn)?;
        Ok(conn)
    }

    /// 注入档埋点（`MemoryStore::recall_quiet` 的**成功路径**调用）。
    ///
    /// 打不开库时不许走到这里：那种情况下连 `n_candidates` 都取不到，记一行 0 会把
    /// 「库不可用」冒充成「库是空的」= 指标造假（判据
    /// `unavailable_memory_store_does_not_write_a_row`）。
    ///
    /// ⚠ `too_many_arguments` 在这里**带理由允许**而不是收束：Q10 加第七个参数
    /// `breakdown` 之后正好过界。两种收束法都赔得更多——① 把 [`Row`] 开放给调用方，
    /// 等于把 `mode` 这个"哪一档"的身份字段也交出去（那样一行既不在部分唯一索引的
    /// 谓词里、又带着 `OR IGNORE`，去重静默失效而没人报错）；② 再造一个与 `Row`
    /// 逐字段重复的入参结构 = 同一份列形状两个出处，正是本仓立过纪律要挡的漂移。
    /// 这条 lint 在仓里的既有处置同样是带理由的 `allow`（`organize/mod.rs` 三处、
    /// `runner/mod.rs`、`llm/client.rs` 两处、`commands.rs`）。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn note_inject(
        &self,
        session_key: &str,
        scope_id: &str,
        n_candidates: usize,
        returned: usize,
        keys: &[String],
        elapsed: Duration,
        breakdown: Option<&str>,
    ) -> Result<(), String> {
        self.insert_row(
            Row {
                mode: MODE_INJECT,
                session_key,
                scope_id,
                n_candidates,
                returned,
                keys,
                elapsed,
                breakdown,
            },
            Dedup::ByKeyside,
        )
    }

    /// 检索档埋点（Q1b·(b)：`memory_read` / `memory_search` / `memory_project_history`
    /// 的**成功回执**，由 [`crate::recall_receipt`] 在工具本体返回后调用）。
    ///
    /// 与注入档的四处不同，都是"检索本来就是一次一次发生的"：
    /// 1. `mode` = 工具名（不是 `'inject'`）⇒ 部分唯一索引管不到它，**不去重**，一次成功
    ///    调用一行（判据 `receipts_are_not_deduplicated_unlike_injections`）；
    /// 2. `n_candidates` 恒 **0** —— 工具结果里没有"库内候选总数"这个量（`memory_search`
    ///    只给了截断后的 matches），编一个数就是把观测面造假。读取面**别把这里的 0 当成
    ///    「库里没货」**，那是注入档 `n_candidates` 的专属含义；
    /// 3. 走严格 `INSERT`（不带 `OR IGNORE`）⇒ NOT NULL 之类的真错误会如实上抛，不会被
    ///    "冲突也忽略"顺手吞掉。
    /// 4. ⚠ Q10 起 `breakdown` 恒 **NULL**：检索档没有"四信号加权"这一步（工具走的是
    ///    `memory_search`/`memory_read` 那两条 SQL，排序与注入路径无关），所以它**没有**
    ///    可解释回执可给。这里补一份"看着像 breakdown 的东西"就是造假——面板读到 NULL
    ///    时该显示"这一档不参与加权排序"，判据盯着这条不许被后来的批填上假数据。
    pub(crate) fn note_receipt(
        &self,
        tool: &str,
        session_key: &str,
        scope_id: &str,
        returned: usize,
        keys: &[String],
        elapsed: Duration,
    ) -> Result<(), String> {
        self.insert_row(
            Row {
                mode: tool,
                session_key,
                scope_id,
                n_candidates: 0,
                returned,
                keys,
                elapsed,
                breakdown: None,
            },
            Dedup::EveryCall,
        )
    }

    /// 落一行的唯一出口。`dedup` 决定语句形状：`ByKeyside` = `INSERT OR IGNORE`，其冲突源
    /// 只有 `ux_mil_inject` 一个部分唯一索引（其余列全 NOT NULL 且都给了值）；`EveryCall` =
    /// 严格 INSERT。两条语句共用列名与参数顺序，所以列序改动只会红一次。
    fn insert_row(&self, row: Row, dedup: Dedup) -> Result<(), String> {
        let conn = self.open_conn()?;
        let sql = match dedup {
            Dedup::ByKeyside => {
                "INSERT OR IGNORE INTO memory_inject_log (ts, mode, session_key, scope_id, n_candidates, returned, keys, elapsed_us, breakdown) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            }
            Dedup::EveryCall => {
                "INSERT INTO memory_inject_log (ts, mode, session_key, scope_id, n_candidates, returned, keys, elapsed_us, breakdown) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            }
        };
        conn.execute(
            sql,
            params![
                now_secs(),
                row.mode,
                row.session_key,
                row.scope_id,
                row.n_candidates as i64,
                row.returned as i64,
                keys_json(row.keys),
                // u128 → i64 钳制：真实时长远不到上限，但类型转换不许静默截断。
                row.elapsed.as_micros().min(i64::MAX as u128) as i64,
                row.breakdown,
            ],
        )
        .map_err(|e| format!("recall log insert: {e}"))?;
        Ok(())
    }

    /// 空块率 + 注入产出率（L0 稿 §2.3(c) 的两个派生指标）。读取点 = Q10 的
    /// `memory_inject_view`（管理页顶部那一行汇总），豁免因此摘掉。
    pub(crate) fn stats(&self) -> Result<InjectStats, String> {
        let conn = self.open_conn()?;
        let (injections, empty_injections): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(returned = 0), 0) FROM memory_inject_log WHERE mode = ?1",
                [MODE_INJECT],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| format!("recall log stats: {e}"))?;
        let injected =
            key_set(&conn, true).map_err(|e| format!("recall log injected keys: {e}"))?;
        let retrieved =
            key_set(&conn, false).map_err(|e| format!("recall log retrieved keys: {e}"))?;
        let yielded = injected.iter().filter(|k| retrieved.contains(*k)).count();
        Ok(InjectStats {
            injections: injections as usize,
            empty_injections: empty_injections as usize,
            store_empty_ratio: (injections > 0)
                .then(|| empty_injections as f64 / injections as f64),
            injected_keys: injected.len(),
            yielded_keys: yielded,
            injection_yield: (!injected.is_empty()).then(|| yielded as f64 / injected.len() as f64),
        })
    }

    /// 最近 N 行（倒序）。生产读取点 = Q10 的 `memory_inject_view`（管理页只读解释区块），
    /// 测试也用它断言。
    pub(crate) fn recent(&self, limit: i64) -> Result<Vec<RecallRow>, String> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT ts, mode, session_key, scope_id, n_candidates, returned, keys, breakdown
                 FROM memory_inject_log ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| format!("recall log recent: {e}"))?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(RecallRow {
                    ts: r.get(0)?,
                    mode: r.get(1)?,
                    session_key: r.get(2)?,
                    scope_id: r.get(3)?,
                    n_candidates: r.get(4)?,
                    returned: r.get(5)?,
                    keys: parse_keys(&r.get::<_, String>(6)?),
                    breakdown: r.get::<_, Option<String>>(7)?,
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

/// 读出某一档出现过的 key **去重集合**（`injection_yield` 的两个侧）。
///
/// `inject_rows = true` 取注入档；false 取**全部检索档**（`mode <> 'inject'`）。后半句是
/// 有意的：不给分子配"哪些工具算 key-bearing"的白名单——白名单会漂（每加一个检索工具就得
/// 改 SQL，忘了改就是静默少算），而 `keys` 为空表的新工具对交集**零贡献**（这正是
/// `memory_project_history` 的情形），所以"全档并集"既不需要维护清单也不会污染读数。
fn key_set(conn: &Connection, inject_rows: bool) -> Result<BTreeSet<String>, rusqlite::Error> {
    let sql = if inject_rows {
        "SELECT keys FROM memory_inject_log WHERE mode = ?1"
    } else {
        "SELECT keys FROM memory_inject_log WHERE mode <> ?1"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([MODE_INJECT], |r| r.get::<_, String>(0))?;
    let mut out = BTreeSet::new();
    for row in rows {
        out.extend(parse_keys(&row?));
    }
    Ok(out)
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
