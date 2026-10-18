//! **Memory-Inject 的检索源**（台账 T33）：`impl MemorySource for MemoryStore` 的
//! 全部方法（现役 `recall` / 学习档 `recall_relevant` / 缓存键 `scope_id` / 归属告知
//! `context_line` / 交接提醒段 `repo_context` / 项目索引 `project_index`）、组装点便利
//! 函数 `memory_source`，以及只被 `context_line` 用的 `today_local`——它的唯一调用点
//! 就在本片，所以跟着搬，不留第二个出处。
//!
//! 本片是 trait 实现，整块自包含（原文那对花括号就在此起此落，不需要补外壳）。用的都是
//! 别片已抬到 `pub(super)` 的入口（`open_conn` / `recall_quiet` / `log_inject_keys` 与
//! 那四个字段），加上原本就 `pub(crate)` 的 `md_index_body` / `index_rows`；
//! 没有一处为"方便"而抬宽。

use std::sync::Arc;

use kernel_agent::{MemoryEntry, MemorySource, RecallQuery};

use crate::timeutil::now_secs;

use super::store::MemoryStore;

/// 本地日期 ISO 形（`YYYY-MM-DD`，`<environment>` Today 行用）。chrono
/// 已在依赖树（tauri 传递引入，声明直依赖零新增编译成本）；取不到按
/// epoch UTC 如实（不发明时区）。
fn today_local() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

/// Memory-Inject 的检索源（注册进 agent/request 决策环）。v2.136 裁定：
/// 注入只取全局层（用户层面的喜好/全局要求；项目层事实不自动注入，模型
/// memory_read / memory_search 按需查）。㉘·A·fix 另供两件：`scope_id`
/// （预取缓存键成分——归属变更自动重算）与 `context_line`（归属会话告知
/// 模型当前项目根，"看下这个项目"不再靠旧记忆猜路径）。
/// 队列 Q9 起**覆写 `recall_relevant`**：排序真源从"SQL 的 `updated_at DESC`"
/// 变成"候选集内存四信号加权"（`memory_learn/`，Q10 起目录化），`recall` 本体保留不动
/// = 学习档关掉时的现役参照。
impl MemorySource for MemoryStore {
    fn recall(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry> {
        // Q1（L0）：session_id 现在透传给 recall_quiet，但**只作观测字段**——
        // 召回恒取全局层（recall_quiet 头注的 v2.136 裁定），交付的条目集合
        // 与改前逐字节一致（等价判据在 memory/tests.rs）。
        self.recall_quiet(session_id, limit)
            .into_iter()
            .map(|r| MemoryEntry {
                key: r.key,
                value: r.value,
                tags: r.tags,
                updated_at: r.updated_at,
                // 只取全局层，层标恒 global（层标渲染在内核侧保留）
                scope: "global".into(),
            })
            .collect()
    }

    /// 队列 Q9（ML 稿 §3.3 排序 + §3.4 强化 = 稿内 L2/L3 合一批）：四信号现算
    /// 加权排序 + 交付即写回。打分与写回的**全部逻辑在 `memory_learn/` 模块**，
    /// 这里只做装配、连接与观测（同一进程内一次连接读完就排，不重开库）。
    ///
    /// ⚠ **关掉学习档走的是 `Self::recall` 本体**，不是"另写一份等价的老 SQL"：
    /// 稿 §5.4 要求 `learnEnabled=false` 逐字节回到现役，这条由**调用图**给出而不是
    /// 由测试近似——现役路径只有一个实现，新路径无法在它身上偷偷改点什么还声称等价。
    ///
    /// ⚠ **订正：写回粒度是"每会话一次"而不是稿 §3.4 说的"每次 `recall()` 调用"**。
    /// 桌面链的 `MemoryInject` 带会话预取缓存（M1 纪律：逐轮动态检索会改写
    /// system 前缀、命中归零），一个会话只有**首查**那一次真走到这里 ⇒ `hit_count`
    /// 的增长单位是**会话**不是轮次，同一会话追问十轮记忆只 +1。方向上是好事
    /// （稿 §6.1 担心的"检索即自我强化"被从 ×轮 压到 ×会话），但必须说破：
    /// A/B 时看到的 `hit_count` 不能按轮次解释，`memory_inject_log` 里一个会话
    /// 也只有一行注入记录（Q1 的既有形状）。
    fn recall_relevant(
        &self,
        session_id: Option<&str>,
        limit: usize,
        query: &RecallQuery,
    ) -> Vec<MemoryEntry> {
        if !query.weights.enabled {
            return self.recall(session_id, limit);
        }
        let started = std::time::Instant::now();
        let now = now_secs();
        let learned: Result<
            (Vec<crate::memory_learn::Scored>, Vec<String>, usize, String),
            String,
        > = (|| {
            let conn = self.open_conn()?;
            // n_candidates 恒取**全量** COUNT（与 recall_quiet 同口径），不是候选
            // 上限：Q1 那条指标的分子分母不能随本刀换定义。
            let n_candidates = crate::memory_learn::count_global(&conn)?;
            let delivered = crate::memory_learn::rank(
                crate::memory_learn::candidates(&conn)?,
                query,
                limit,
                now,
            );
            // 写回与埋点吃**同一个 keys 变量**：交付集合、`hit_count` 增量、
            // `memory_inject_log.keys` 三者不可能分叉（分叉过一次就是"日志说注入了
            // 三条、库里只加了两条"这种查不出的账）。
            let keys: Vec<String> = delivered.iter().map(|s| s.cand.key.clone()).collect();
            crate::memory_learn::reinforce(&conn, &keys, now);
            // ⚠ Q10（L4）：四信号回执**只走观测旁路**——它进 `memory_inject_log.breakdown`
            // 由管理页回放，**不进** `MemoryEntry`（那等于把机器状态摆进 system，还花
            // token；上批判据 S10 钉的那条原样在场）。回执与 keys 同样取自 `delivered`
            // 这同一个变量 ⇒ "日志里的分数"与"真正交付的那几条"不可能各说一套。
            let breakdown = crate::memory_learn::breakdown_json(&delivered, query);
            Ok((delivered, keys, n_candidates, breakdown))
        })();
        match learned {
            Ok((rows, keys, n_candidates, breakdown)) => {
                self.log_inject_keys(session_id, n_candidates, &keys, started, Some(&breakdown));
                rows.into_iter()
                    .map(|s| MemoryEntry {
                        key: s.cand.key,
                        value: s.cand.value,
                        tags: s.cand.tags,
                        updated_at: s.cand.updated_at,
                        // 与现役 recall 同一个层标口径：注入只取全局层。
                        scope: "global".into(),
                    })
                    .collect()
            }
            Err(e) => {
                // 与 `recall_quiet` 共用 `warned`：两处失效原因是**同一个** open_conn
                // （库打不开/查询失败），各开一位会让其中一边在真故障时静默两次
                // 告警；project_index 那位独立 ONCE 的理由（失效原因不同）在这里不成立。
                // 同样**不记埋点**：连 n_candidates 都取不到，记 0 会把"库不可用"
                // 冒充成"库是空的"（指标造假）。
                if !self.warned.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    tracing::warn!("memory recall unavailable（记忆注入降级为空）: {e}");
                }
                Vec::new()
            }
        }
    }

    fn scope_id(&self, session_id: Option<&str>) -> String {
        // v2.136：提醒段改静态文本后，交接文件 mtime 指纹退出缓存键——
        // 人改 .Locowrk/ 文件不再触发记忆块重算（正文本就不注入）。
        self.scope_of_session(session_id)
    }

    fn context_line(&self, session_id: Option<&str>) -> Option<String> {
        let ws = self.scope_of_session(session_id);
        if ws.is_empty() {
            return None;
        }
        // 提示词分层设计 §4.2/§5.3（v2.139 `<environment>` 内文）：项目根 +
        // 日期/平台 + 按需查路径（"list/read it first" 退役——「背景不是
        // 议程」）。日期/平台随块同吃会话预取 = 会话锁死（首请求定格，
        // 跨天不发提醒——桌面会话单日为主，宁缺勿假）。
        Some(format!(
            "Project workspace: {ws} — \"this project\" in the user's message \
             refers to this directory.\n\
             Today: {}. Platform: {}.\n\
             Look up project files when the request touches them; treat paths \
             recalled from memory as hints to verify, not facts.",
            today_local(),
            std::env::consts::OS
        ))
    }

    /// v2.136 提醒段（项目侧全提醒制）：归属会话注入 `<repo>/.Locowrk/`
    /// **静态提醒文本**（目录里有什么、在什么节点读/更新）——正文不再
    /// 注入，项目记忆由当前工作的主模型按需用文件工具查看与维护。首次
    /// 调用顺手引导建目录（幂等，每仓库一次）；开关关 / 未归属 / 非目录
    /// = None（组装零变化）。
    fn repo_context(&self, session_id: Option<&str>) -> Option<String> {
        if !(self.repo_on)() {
            return None;
        }
        let repo = self.repo_root(session_id)?;
        {
            let mut set = self.repo_ensured.lock().expect("repo ensured poisoned");
            if set.insert(repo.clone()) {
                if let Err(e) = crate::repo_context::ensure_layout(&repo) {
                    tracing::warn!("交接目录引导失败 {}: {e}", repo.display());
                }
            }
        }
        crate::repo_context::build(&repo)
    }

    /// Q2（MR 稿 §4.1）：项目记忆**索引**（指针行，不含正文）。
    ///
    /// 未归属会话 = 没有"本项目"可言 ⇒ `None`（整节缺席）。S1 刀②b-3 起**读源 = md**，
    /// 库里现合成那一档只在"人类层没接线"时才有资格（暂态，刀②c 取消）；md 里
    /// 没有指针行就如实缺席，**不回写文件、也不去库里补一份不再是真源的副本**。
    /// 取数失败 = `None` + 一次告警，绝不合成一条"看起来是空索引"的块——
    /// 那会把"读不到"骗成"没东西可注入"。
    fn project_index(&self, session_id: Option<&str>) -> Option<String> {
        let ws = self.scope_of_session(session_id);
        if ws.is_empty() {
            return None;
        }
        // 刀②b-3：**两档的关系反过来了**。原先"md 优先、空壳就退库里现合成"是双写
        // 时代的人可编辑优先；md 成真源之后"文件里没指针行"就是这个项目确实没记过，
        // 如实缺席。库里那一档现在只剩一种资格——人类层没接线（单测 / 降级启动），
        // 那一档由刀②c 取消。判据因此是 `files_face()` 有没有句柄，不是 md 内容
        // 像不像有货（后者正是上一版会"读到空气"的地方：本机实测 TITY 目录里
        // 根本没有 `MEMORY.md`，另外三个项目都只有空壳标题）。
        if self.files_face().is_none() {
            return match self.index_rows(&ws, crate::memory_index::MAX_INDEX_LINES + 1) {
                Ok(rows) => crate::memory_index::from_rows(&rows, now_secs()),
                Err(e) => {
                    // 每 step 都会走到这里，只告警一次（`warned` 那位是给 recall 的，
                    // 不复用——两边失效原因不同，混用会让其中一边永远静默）
                    static ONCE: std::sync::Once = std::sync::Once::new();
                    ONCE.call_once(|| {
                        tracing::warn!("memory index unavailable（项目索引注入降级为缺席）: {e}")
                    });
                    None
                }
            };
        }
        crate::memory_index::from_md(self.md_index_body(&ws).as_deref())
    }
}

/// 组装点便利：Arc 化并作为 trait 对象交给 MemoryInject。
pub fn memory_source(store: &Arc<MemoryStore>) -> Arc<dyn MemorySource> {
    Arc::clone(store) as Arc<dyn MemorySource>
}
