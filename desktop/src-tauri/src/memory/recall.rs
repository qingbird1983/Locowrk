//! **全局层召回与注入埋点**（台账 T33）：`recall_quiet`（v2.136 裁定后只取全局层，
//! 打不开库只告警一次、注入不拦聊天）与两层埋点 `log_inject` / `log_inject_keys`。
//! 埋点拆成两层而不是改签名，是为了让现役调用点一字未动——那段理由在原签名上方的注释里。
//!
//! `recall_quiet` 与 `log_inject_keys` 抬成 `pub(super)`：前者是 `source.rs` 里 trait
//! 实现的现役通路，后者是学习档（Q9）绕过 SQL 排序之后仍要记同一本账的入口。`log_inject`
//! 只有本片调用，保持私有。"库不可用不许冒充库是空的"那条不记埋点的纪律，
//! S1 刀②b-2 起抽成了可单测的纯函数 `inject_loggable`（并扩到"读数不完整同样不记"）。
//!
//! `impl MemoryStore {` 是本片自补的语法外壳（刀②b-2 起它上面还多了两个模块级项：
//! `RecallOutcome` 与 `inject_loggable`——Rust 不许在 `impl` 里定义类型，实测踩过）；
//! 末行的顶格闭合不是补的，它就是原文 617 行那个合上整个 impl 的 `}`，正好归本片。

use rusqlite::params;

use super::shapes::MemoryRow;
use super::store::MemoryStore;

/// 一次召回的读数。**两条读源共用这一个形状**：S1 刀②b-2 起 md 与 sqlite 各一支，
/// 埋点口径若各写一份，"不许冒充源是空的"那条纪律就只在一支里活着。
pub(super) struct RecallOutcome {
    pub(super) rows: Vec<MemoryRow>,
    pub(super) n_candidates: usize,
    /// true = 有一部分没能读出来（人类层里有扫不动的条目）。
    pub(super) partial: bool,
}

/// 埋点该不该记：**只有完整读数才记**。
///
/// 记一条 `n_candidates=0` 会把「源不可用」冒充成「源是空的」= 指标造假；
/// `partial` 是同一个错的另一半——那一轮真正能被模型看到的候选比这个数多，
/// 落进表的 `returned / n_candidates` 就系统性偏高。宁缺毋伪。
/// 抽成模块级纯函数（Rust 不许在 `impl` 里定义类型）是为了让它有机械证明：这条
/// 判据决定的是"要不要写一行日志"，没有断言的话实现漂成"总是记"没人会看见。
pub(super) fn inject_loggable(o: &Result<RecallOutcome, String>) -> bool {
    matches!(o, Ok(x) if !x.partial)
}

impl MemoryStore {
    /// 降级告警只说一次（每 step 都召回，不打闸门就是刷日志）。
    ///
    /// `warned` 这个闸门现在服务**两类**降级（源不可用 / 读数不完整）⇒ 收成一处，
    /// 两处各写一份 `swap` 的话，第二类会被第一类永久静音。
    fn warn_once(&self, msg: &str) {
        if !self.warned.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("{msg}");
        }
    }

    pub(super) fn recall_quiet(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryRow> {
        // v2.136 裁定「项目侧全提醒制」：注入只取**全局层**（''，用户层
        // 面的喜好/全局要求，保持简短）——项目层事实不自动注入（由模型
        // memory_read / memory_search 按需查），所以召回与会话归属无关。
        // Q1（L0）：session_id 现在会往下传，但**只作观测字段**（日志的
        // 会话键与键位），召回集合与改前同源。
        let started = std::time::Instant::now();
        let outcome = self.recall_once(limit);
        if inject_loggable(&outcome) {
            // 上一条判据已经保证这是 Ok 且完整
            if let Ok(o) = &outcome {
                self.log_inject(session_id, o.n_candidates, &o.rows, started);
            }
        } else if let Err(e) = &outcome {
            self.warn_once(&format!(
                "memory recall unavailable（记忆注入降级为空）: {e}"
            ));
        } else {
            self.warn_once("记忆 md 召回不完整（有扫不动的条目，本次埋点不记）");
        }
        outcome.map(|o| o.rows).unwrap_or_default()
    }

    /// 取一次全局层。人类层可用 ⇒ **读 md，一条 SQL 都不发**；还能落到下面那条 SQL
    /// 通路的只剩"未接线（单测）/ 总开关关"，那一档由刀②c 取消。
    ///
    /// 两条支路都让 `n_candidates` 与交付条数**同源于同一次读数**：原注释说的是
    /// "同一次连接"，换了读源之后是"同一次扫描"——理由一模一样，分两次取会在中间被
    /// 写入打乱，读出的「有多少 / 这次给了多少」就不是同一时刻的账。
    fn recall_once(&self, limit: usize) -> Result<RecallOutcome, String> {
        if let Some(files) = self.files_face() {
            let scan = plugin_api::memlayout::iter_facts(files.root());
            // 行形状只有一份装配（`shapes.rs` 的 `From<&StoredFact>`），与管理页列举同源；
            // 全局层的判据是**读回来的 workspace = ''**，与下面那条 `WHERE workspace = ''`
            // 同一个键位口径（写侧两边都吃归一串，所以''这一维逐字对得上）。
            let global: Vec<_> = scan
                .facts
                .iter()
                .filter(|f| f.workspace.is_empty())
                .collect();
            return Ok(RecallOutcome {
                rows: global
                    .iter()
                    .take(limit)
                    .map(|f| MemoryRow::from(*f))
                    .collect(),
                n_candidates: global.len(),
                partial: scan.unreadable > 0,
            });
        }
        let conn = self.open_conn()?;
        let n_candidates: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE workspace = ''",
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("memory recall count: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT workspace, key, value, tags, source, updated_at FROM memories
                 WHERE workspace = ''
                 ORDER BY updated_at DESC, key LIMIT ?1",
            )
            .map_err(|e| format!("memory recall: {e}"))?;
        let rows = stmt
            .query_map(params![limit as i64], Self::row)
            .map_err(|e| format!("memory recall: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("memory recall: {e}"))?;
        Ok(RecallOutcome {
            rows,
            n_candidates: n_candidates as usize,
            // sqlite 支路要么整取要么 Err，不存在"取到一部分"。这一格恒 false 不是偷懒，
            // 是两支路的故障形状本身不同：文件可以逐条坏，库是整开整不开。
            partial: false,
        })
    }

    /// Q1（L0）注入侧埋点。未接线 = 直接返回；写失败由 `UsageRecorder` 内部 warn。
    ///
    /// 键位在这里现算（`scope_of_session` = 旁车一次读取）：`MemoryInject` 那边为了
    /// 预取缓存已经算过一次，这里重复一次同成本档——**不为一个日志字段去改内核侧
    /// trait 形状**。
    fn log_inject(
        &self,
        session_id: Option<&str>,
        n_candidates: usize,
        rows: &[MemoryRow],
        started: std::time::Instant,
    ) {
        let keys: Vec<String> = rows.iter().map(|r| r.key.clone()).collect();
        // 现役路径没有四信号可交代 ⇒ `None`（落 NULL）。这不是"忘了填"：学习档关掉时
        // 排序发生在 SQL 的 `ORDER BY updated_at DESC` 里，那一步根本不算分。
        self.log_inject_keys(session_id, n_candidates, &keys, started, None);
    }

    /// 埋点的键位版（Q9 学习路径给的是 `memory_learn::Candidate`，不是管理页
    /// 行形状）。**拆成两层而不是改上面那个签名**：现役 `recall_quiet` 的调用点
    /// 因此一字未动，「关掉学习档 = 逐字节回到现役」这条判据里连观测面都算得上。
    ///
    /// Q10 起多带一个 `breakdown`（四信号回执 JSON，`None` = 这一档没有分数）。
    pub(super) fn log_inject_keys(
        &self,
        session_id: Option<&str>,
        n_candidates: usize,
        keys: &[String],
        started: std::time::Instant,
        breakdown: Option<&str>,
    ) {
        let Some(log) = &self.recall_log else {
            return;
        };
        let scope = self.scope_of_session(session_id);
        // 无会话传空串而不是 NULL：部分唯一索引里 NULL ≠ NULL，用 NULL 会让
        // 「无会话的注入」每 step 插一条，去重直接失效（recall_log/mod.rs 不变量 3）。
        if let Err(e) = log.note_inject(
            session_id.unwrap_or_default(),
            &scope,
            n_candidates,
            keys.len(),
            keys,
            started.elapsed(),
            breakdown,
        ) {
            tracing::warn!("recall observability: {e}");
        }
    }
}
