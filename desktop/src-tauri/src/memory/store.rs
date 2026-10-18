//! `MemoryStore` 的**结构与装配面**（台账 T33）：字段、构造、三条接线（召回观测 /
//! 交接注入 / 人类层 md）、作用域解析、md 双写与项目索引取数，外加所有分片共用的两件
//! 底层件——`open_conn`（开连接 + 建表 + 两侧迁移都挂在这一条线上）与行装配 `row`。
//!
//! 本片对同类型的其余分片（`crud` / `reconcile` / `recall` / `source`）给出四个
//! `pub(super)` 入口：`open_conn`、`row`、`md_write`、`md_delete`；跨片读到的四个字段
//! `warned` / `repo_on` / `repo_ensured` / `recall_log` 同口径抬升，其余字段保持私有
//! （只有本片碰得到）。抬升只到 `memory` 子树，出不了模块 ⇒ 模块外的公开面与此前逐字相同。
//!
//! 末尾那条合上的花括号是**本片自补的语法外壳**：`impl MemoryStore` 按面分进四个文件，
//! 原文那一对花括号只属于"开到这里"的这一片，剩下的由本片自己收。

use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection};

use crate::workspace::{scope_key, WorkspaceResolver};

use super::schema::{ensure_learning_schema, ensure_workspace_schema};
use super::shapes::{FactLine, MemoryRow, CREATE_TABLE};

pub struct MemoryStore {
    dir: PathBuf,
    /// 会话 → 项目根解析（workspace.rs 单一真源；recall 按键位过滤用）。
    workspace_of: WorkspaceResolver,
    /// 检索失败只告警一次（每 step 都 recall，别刷日志）。
    pub(super) warned: AtomicBool,
    /// F1 人类层（md 双写，文件化设计 §3）：sqlite 为真源，宿主写
    /// 路径（store write / 管理页 update·delete·clear）成功后同步 md；
    /// 未接线（单测）= 纯 sqlite。
    files: Option<Arc<crate::memory_files::MemoryFiles>>,
    /// 文件化总开关（settings `memory.files` 闭包；缺省无接线 = 恒关）。
    files_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// F2 交接注入开关（settings `memory.repoContext` 闭包；缺省恒关）。
    pub(super) repo_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// 已引导过的仓库（每进程每仓库一次 ensure_layout，幂等也兜底防抖）。
    pub(super) repo_ensured: Mutex<std::collections::HashSet<PathBuf>>,
    /// Q1a（L0 注入侧可观测性）：埋点落库句柄（`usage.sqlite3` 旁挂的
    /// `memory_inject_log`）。**未接线（单测、降级启动）= 不记**，召回行为逐字不变。
    /// 句柄类型是 `recall_log::RecallLog` 而不是 `usage::UsageRecorder`：统计写入器
    /// 不该兼管记忆观测（L0 稿 §3.2「命中与工具热度混起来会互相污染」）。
    pub(super) recall_log: Option<Arc<crate::recall_log::RecallLog>>,
}

impl MemoryStore {
    pub fn new(dir: &Path, workspace_of: WorkspaceResolver) -> Self {
        Self {
            dir: dir.to_path_buf(),
            workspace_of,
            warned: AtomicBool::new(false),
            files: None,
            files_on: Arc::new(|| false),
            repo_on: Arc::new(|| false),
            repo_ensured: Mutex::new(std::collections::HashSet::new()),
            recall_log: None,
        }
    }

    /// 接线召回观测（Q1 / L0，state 组装点调用）：注入侧每次交付记一行
    /// `memory_inject_log`。**只记不改**——召回的 SQL、条数、字节全部与改前同源。
    pub fn with_recall_log(mut self, log: Arc<crate::recall_log::RecallLog>) -> Self {
        self.recall_log = Some(log);
        self
    }

    /// 接线交接注入（F2，state 组装点调用）：settings `memory.repoContext`
    /// 闭包——归属会话注入 `<repo>/.Locowrk/` 交接段 + 引导建目录。
    pub fn with_repo_context(mut self, on: Arc<dyn Fn() -> bool + Send + Sync>) -> Self {
        self.repo_on = on;
        self
    }

    /// 会话的仓库根（归属路径 = 交接面落点）。未归属 / 非目录 = None。
    /// v2.139 pub：指令文件层（instruction_files.rs）同一真源找项目根
    /// AGENTS.md。
    pub fn repo_root(&self, session_id: Option<&str>) -> Option<PathBuf> {
        let sid = session_id?;
        let ws = (self.workspace_of)(sid)?;
        if ws.is_empty() {
            return None;
        }
        let repo = PathBuf::from(&ws);
        repo.is_dir().then_some(repo)
    }

    /// 接线人类层（state 组装点调用）：md 句柄 + 总开关闭包。
    pub fn with_files(
        mut self,
        files: Arc<crate::memory_files::MemoryFiles>,
        on: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        self.files = Some(files);
        self.files_on = on;
        self
    }

    /// 人类层句柄的**唯一取用档**：接线了**且**总开关开着才给句柄。
    ///
    /// 为什么现在收：这句判断此前散在三处（`md_write` / `md_delete` /
    /// `md_index_body` 各写一份 `(Some(files), true)` 或 `!(files_on)()`），
    /// 刀②b 的读侧是第四个消费者 ⇒ 不收成一处，"人类层到底开没开"就有四本账。
    /// ⚠ 返回 `None` 的语义是"这一刀还走 sqlite"，那是**暂时状态**：刀②c 要把它
    /// 改成 `Err`（真源 = md 之后，读不到文件就是读不到，不许静默换源）。
    pub(super) fn files_face(&self) -> Option<&crate::memory_files::MemoryFiles> {
        if !(self.files_on)() {
            return None;
        }
        self.files.as_deref()
    }

    /// 双写一条事实 md（失败只 warn——人类层降级不破契约）。
    pub(super) fn md_write(
        &self,
        ws: &str,
        key: &str,
        value: &str,
        tags: &str,
        source: &str,
        at: i64,
    ) {
        let Some(files) = self.files_face() else {
            return;
        };
        if let Err(e) = files.write_fact(ws, key, value, tags, source, at) {
            tracing::warn!("记忆 md 同步失败 `{key}`: {e}");
        }
    }

    /// 同步删除一条事实 md。
    pub(super) fn md_delete(&self, ws: &str, key: &str) {
        let Some(files) = self.files_face() else {
            return;
        };
        if let Err(e) = files.delete_fact(ws, key) {
            tracing::warn!("记忆 md 删除同步失败 `{key}`: {e}");
        }
    }

    /// 会话（可缺）→ 键位（未归属 = ""）。context_line / scope_id 共用。
    pub fn scope_of_session(&self, session_id: Option<&str>) -> String {
        scope_key(session_id.and_then(|s| (self.workspace_of)(s)).as_deref())
    }

    /// 项目 `MEMORY.md` 投影全文（Q2 索引注入的**优先档**，人可编辑的那份）。
    ///
    /// `None` = 未接线 / 文件化开关关 / 项目键位为空 / 文件读不到。注意
    /// "文件在场但只有 `# 记忆索引` 标题"也归入"用不上"——那是本机实测的常态
    /// （见 `memory_index` 头注），判断在 `memory_index::from_md` 里做。
    pub(crate) fn md_index_body(&self, workspace: &str) -> Option<String> {
        if workspace.is_empty() {
            return None;
        }
        let files = self.files_face()?;
        // 家目录经 memlayout 单点算式（Q15 收口：这里此前自己拼过一份
        // `projects/<可读名-哈希>`，那是同一句话的第三个出处）。`.ok()?`
        // 而不是 unwrap：算不出来 = 这一节缺席，按同函数头注的降级口径走。
        let path = plugin_api::memlayout::project_dir(files.root(), workspace)
            .ok()?
            .join("MEMORY.md");
        std::fs::read_to_string(path).ok()
    }

    /// 某键位的**指针级**行（Q2 索引兜底档；`limit` 由调用方给 `上限 + 1`，
    /// 用来判"还有更多没进索引"）。
    ///
    /// 打不开库 = `Err` 上抛给调用方按"整节缺席"降级——**不许**把"读不到"
    /// 合成成"库里没货"的空索引（Q1a 观测档「库不可用不许冒充库是空的」同纪律）。
    /// SQL 里不带 `value` 列 = 「正文永不自动进上下文」的机器保证。
    pub(crate) fn index_rows(
        &self,
        workspace: &str,
        limit: usize,
    ) -> Result<Vec<FactLine>, String> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT key, tags, updated_at FROM memories WHERE workspace = ?1
                 ORDER BY updated_at DESC, key LIMIT ?2",
            )
            .map_err(|e| format!("memory index rows: {e}"))?;
        let rows = stmt
            .query_map(params![workspace, limit as i64], |r| {
                Ok(FactLine {
                    key: r.get(0)?,
                    tags: r.get(1)?,
                    updated_at: r.get(2)?,
                })
            })
            .map_err(|e| format!("memory index rows: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("memory index rows: {e}"))?;
        Ok(rows)
    }

    /// 按调用开连接、用完即弃（与插件同款）；建表 + 旧库迁移幂等。
    pub(super) fn open_conn(&self) -> Result<Connection, String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("memory db dir `{}`: {e}", self.dir.display()))?;
        let conn = Connection::open(self.dir.join("memory.sqlite3"))
            .map_err(|e| format!("memory db open: {e}"))?;
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(|e| format!("memory db busy_timeout: {e}"))?;
        conn.execute_batch(CREATE_TABLE)
            .map_err(|e| format!("memory db init: {e}"))?;
        ensure_workspace_schema(&conn)?;
        ensure_learning_schema(&conn)?;
        Ok(conn)
    }

    pub(super) fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRow> {
        Ok(MemoryRow {
            workspace: r.get(0)?,
            key: r.get(1)?,
            value: r.get(2)?,
            tags: r.get(3)?,
            source: r.get(4)?,
            updated_at: r.get(5)?,
        })
    }
}
