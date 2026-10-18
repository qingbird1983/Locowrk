//! **全局层召回与注入埋点**（台账 T33）：`recall_quiet`（v2.136 裁定后只取全局层，
//! 打不开库只告警一次、注入不拦聊天）与两层埋点 `log_inject` / `log_inject_keys`。
//! 埋点拆成两层而不是改签名，是为了让现役调用点一字未动——那段理由在原签名上方的注释里。
//!
//! `recall_quiet` 与 `log_inject_keys` 抬成 `pub(super)`：前者是 `source.rs` 里 trait
//! 实现的现役通路，后者是学习档（Q9）绕过 SQL 排序之后仍要记同一本账的入口。`log_inject`
//! 只有本片调用，保持私有。"库不可用不许冒充库是空的"那条不记埋点的纪律也在原处。
//!
//! 首行 `impl MemoryStore {` 是本片自补的语法外壳；末行的顶格闭合不是补的，它就是原文
//! 617 行那个合上整个 impl 的 `}`，正好归本片。

use rusqlite::params;

use super::shapes::MemoryRow;
use super::store::MemoryStore;

impl MemoryStore {
    pub(super) fn recall_quiet(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryRow> {
        // v2.136 裁定「项目侧全提醒制」：注入只取**全局层**（''，用户层
        // 面的喜好/全局要求，保持简短）——项目层事实不自动注入（由模型
        // memory_read / memory_search 按需查），所以召回与会话归属无关。
        // Q1（L0）：session_id 现在会往下传，但**只作观测字段**（日志的
        // 会话键与键位），查询条件一个字符没动 = 召回结果与改前同源。
        let started = std::time::Instant::now();
        let recalled: Result<(Vec<MemoryRow>, usize), String> = (|| {
            let conn = self.open_conn()?;
            // n_candidates 与交付条数**同源于同一次连接**：分两次查会在中间被写入
            // 打乱，读出的「库里有多少 / 这次给了多少」就不是同一时刻的账。
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
            Ok((rows, n_candidates as usize))
        })();
        match recalled {
            Ok((rows, n_candidates)) => {
                self.log_inject(session_id, n_candidates, &rows, started);
                rows
            }
            Err(e) => {
                // 打不开库/查询失败：连 n_candidates 都取不到，**不记**（记一条
                // n_candidates=0 会把「库不可用」冒充成「库是空的」= 指标造假）。
                if !self.warned.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    tracing::warn!("memory recall unavailable（记忆注入降级为空）: {e}");
                }
                Vec::new()
            }
        }
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
