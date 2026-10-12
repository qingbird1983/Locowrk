//! 记忆库宿主侧访问（⑪）：`<数据根>/memory.sqlite3` 归 memory-sqlite
//! 插件所有（schema 权威在两侧同源迁移）；本模块做两类宿主自有访问——
//! ① `MemorySource` 实现：Memory-Inject（kernel-agent）每 step 的检索源。
//!    不走工具管道（否则每 step 一次 memory_list 会污染使用统计与审计），
//!    直读同一 sqlite 文件（两侧都是短连接，busy_timeout 兜底并发）。
//! ② 记忆管理页的存储支撑（Context and Memory.md §7.2）：列举 / 编辑 /
//!    删除 / 清空——Tauri 命令薄适配到这里，命令层不写 SQL。
//!
//! source 列（explicit | extracted，§4.5 管理页要显示来源）归宿主侧
//! 演进；workspace 列（㉘，项目隔离键位）两侧同形状——建表即带，旧库
//! 由 `ensure_workspace_schema` 整表重建进未归属键位（与插件侧迁移
//! 等价幂等，先跑谁都一样）。
//!
//! **注入面（v2.136 裁定「项目侧全提醒制」）**：Memory-Inject 只取
//! **全局层**（`''` = 用户层面的喜好/全局要求，对一切会话可见；未归属
//! 会话同样只落全局层）——项目层事实不再自动注入，由模型 memory_read /
//! memory_search 按需查；项目记忆文件（`<repo>/.Locowrk/`）以静态提醒段
//! 指路（repo_context），由当前工作的主模型自己读/写/改。管理页是人的
//! 管理面，看全部并透出归属。降级纪律：打不开 = 检索降级空表（仅告警
//! 一次，注入不拦聊天）+ 管理命令报错给 UI；不在启动路径上 panic。

use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;

use kernel_agent::{MemoryEntry, MemorySource};

use crate::timeutil::now_secs;
use crate::workspace::{scope_key, WorkspaceResolver};

/// 与 memory-sqlite 插件同限：key 主键不能无限长，value 防 runaway 写。
pub(crate) const KEY_MAX: usize = 512;
pub(crate) const VALUE_MAX: usize = 256 * 1024;
/// 管理页单次列举上限（页内滚动不分页，200 条对"用户的笔记"足够）。
const LIST_MAX: u32 = 200;

/// 管理页行（Tauri 返回值；snake_case 与既有 IPC 投影一致）。
#[derive(Debug, Clone, Serialize)]
pub struct MemoryRow {
    pub key: String,
    pub value: String,
    pub tags: String,
    /// "explicit" | "extracted"（前端按档位渲染人类文案，§8.1 结构化）。
    pub source: String,
    /// 项目归属（㉘）：会话关联的项目根，`""` = 未归属（§2.3 关联）。
    pub workspace: String,
    pub updated_at: i64,
}

/// 索引注入用的一行（Q2 / MR §4.1）：**只取指针级三件**，`value` 一个字节都不取
/// ——「正文永不自动进上下文」是 §4.1 的硬边界，查询里不带 value 列就是它的机器
/// 保证（要绕过就得改 SQL，而 SQL 有判据盯着）。
pub(crate) struct FactLine {
    pub(crate) key: String,
    pub(crate) tags: String,
    pub(crate) updated_at: i64,
}

/// 与 memory-sqlite 插件同源的权威表形状（迁移等价性依赖两侧一致）。
const CREATE_TABLE: &str = "CREATE TABLE memories (
    workspace  TEXT NOT NULL DEFAULT '',
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    tags       TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL,
    source     TEXT NOT NULL DEFAULT 'explicit',
    PRIMARY KEY (workspace, key)
);";

pub struct MemoryStore {
    dir: PathBuf,
    /// 会话 → 项目根解析（workspace.rs 单一真源；recall 按键位过滤用）。
    workspace_of: WorkspaceResolver,
    /// 检索失败只告警一次（每 step 都 recall，别刷日志）。
    warned: AtomicBool,
    /// F1 人类层（md 双写，文件化设计 §3）：sqlite 为真源，宿主写
    /// 路径（store write / 管理页 update·delete·clear）成功后同步 md；
    /// 未接线（单测）= 纯 sqlite。
    files: Option<Arc<crate::memory_files::MemoryFiles>>,
    /// 文件化总开关（settings `memory.files` 闭包；缺省无接线 = 恒关）。
    files_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// F2 交接注入开关（settings `memory.repoContext` 闭包；缺省恒关）。
    repo_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// 已引导过的仓库（每进程每仓库一次 ensure_layout，幂等也兜底防抖）。
    repo_ensured: Mutex<std::collections::HashSet<PathBuf>>,
    /// Q1a（L0 注入侧可观测性）：埋点落库句柄（`usage.sqlite3` 旁挂的
    /// `memory_inject_log`）。**未接线（单测、降级启动）= 不记**，召回行为逐字不变。
    /// 句柄类型是 `recall_log::RecallLog` 而不是 `usage::UsageRecorder`：统计写入器
    /// 不该兼管记忆观测（L0 稿 §3.2「命中与工具热度混起来会互相污染」）。
    recall_log: Option<Arc<crate::recall_log::RecallLog>>,
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

    /// 双写一条事实 md（失败只 warn——人类层降级不破契约）。
    fn md_write(&self, ws: &str, key: &str, value: &str, tags: &str, source: &str, at: i64) {
        let (Some(files), true) = (self.files.as_ref(), (self.files_on)()) else {
            return;
        };
        if let Err(e) = files.write_fact(ws, key, value, tags, source, at) {
            tracing::warn!("记忆 md 同步失败 `{key}`: {e}");
        }
    }

    /// 同步删除一条事实 md。
    fn md_delete(&self, ws: &str, key: &str) {
        let (Some(files), true) = (self.files.as_ref(), (self.files_on)()) else {
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
        if workspace.is_empty() || !(self.files_on)() {
            return None;
        }
        let files = self.files.as_ref()?;
        let path = files
            .root()
            .join("projects")
            .join(plugin_api::memlayout::project_dir_name(workspace))
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
    fn open_conn(&self) -> Result<Connection, String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("memory db dir `{}`: {e}", self.dir.display()))?;
        let conn = Connection::open(self.dir.join("memory.sqlite3"))
            .map_err(|e| format!("memory db open: {e}"))?;
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(|e| format!("memory db busy_timeout: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                 workspace  TEXT NOT NULL DEFAULT '',
                 key        TEXT NOT NULL,
                 value      TEXT NOT NULL,
                 tags       TEXT NOT NULL DEFAULT '',
                 updated_at INTEGER NOT NULL,
                 source     TEXT NOT NULL DEFAULT 'explicit',
                 PRIMARY KEY (workspace, key)
             );",
        )
        .map_err(|e| format!("memory db init: {e}"))?;
        ensure_workspace_schema(&conn)?;
        Ok(conn)
    }

    fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRow> {
        Ok(MemoryRow {
            workspace: r.get(0)?,
            key: r.get(1)?,
            value: r.get(2)?,
            tags: r.get(3)?,
            source: r.get(4)?,
            updated_at: r.get(5)?,
        })
    }

    /// 管理页列举（全键位）：query 空 = 全量（新→旧），否则 key/value/tags
    /// 子串过滤（LIKE 通配符按字面量，与 memory_search 同纪律）。
    pub fn list(&self, query: &str, limit: u32) -> Result<Vec<MemoryRow>, String> {
        let conn = self.open_conn()?;
        let limit = limit.clamp(1, LIST_MAX);
        if query.is_empty() {
            let mut stmt = conn
                .prepare(
                    "SELECT workspace, key, value, tags, source, updated_at FROM memories
                     ORDER BY updated_at DESC, key LIMIT ?1",
                )
                .map_err(|e| format!("memory list: {e}"))?;
            let rows = stmt
                .query_map(params![limit], Self::row)
                .map_err(|e| format!("memory list: {e}"))?
                .collect::<Result<_, _>>()
                .map_err(|e| format!("memory list: {e}"))?;
            return Ok(rows);
        }
        let pattern = format!(
            "%{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut stmt = conn
            .prepare(
                "SELECT workspace, key, value, tags, source, updated_at FROM memories
                 WHERE key LIKE ?1 ESCAPE '\\' OR value LIKE ?1 ESCAPE '\\'
                    OR tags LIKE ?1 ESCAPE '\\'
                 ORDER BY updated_at DESC, key LIMIT ?2",
            )
            .map_err(|e| format!("memory list: {e}"))?;
        let rows = stmt
            .query_map(params![pattern, limit], Self::row)
            .map_err(|e| format!("memory list: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("memory list: {e}"))?;
        Ok(rows)
    }

    /// 管理页编辑（§7.2 memory_update：只改 value，来源与键名不动）。
    /// 返回 false = 该键位不存在。
    pub fn update_value(&self, workspace: &str, key: &str, value: &str) -> Result<bool, String> {
        if key.is_empty() || key.len() > KEY_MAX {
            return Err(format!("memory key 长度需在 1..={KEY_MAX} 字节"));
        }
        if value.len() > VALUE_MAX {
            return Err(format!("memory value 超过 {VALUE_MAX} 字节上限"));
        }
        let conn = self.open_conn()?;
        let now = now_secs();
        let n = conn
            .execute(
                "UPDATE memories SET value = ?3, updated_at = ?4
                 WHERE workspace = ?1 AND key = ?2",
                params![workspace, key, value, now],
            )
            .map_err(|e| format!("memory update `{key}`: {e}"))?;
        if n > 0 {
            // F1：编辑后按行现值同步 md（取 tags/source/updated_at 真值）
            let row = conn
                .query_row(
                    "SELECT value, tags, source, updated_at FROM memories \
                     WHERE workspace = ?1 AND key = ?2",
                    params![workspace, key],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                        ))
                    },
                )
                .ok();
            if let Some((v, tags, source, at)) = row {
                self.md_write(workspace, key, &v, &tags, &source, at);
            }
        }
        Ok(n > 0)
    }

    /// 写入/覆盖一条记忆（store 基础 upsert）。生产写路径现有三处：
    /// memory-sqlite 插件的 `memory_write` 工具（模型主动写，键位已在
    /// `WsCtx::from_parts` 归一）、管理页 `update_value`，以及 **v2.186 起的
    /// 整理事实卡落库**（`organize/facts.rs`）。键位 = (workspace, key)。
    ///
    /// `source` 三档 = `"explicit"`（用户或模型亲手写）| `"extracted"`
    /// （v2.136 退役的旧自动提取遗留）| `"organized"`（v2.186 = 整理副产物，
    /// 前端 `SourceBadge` 单独一档，用户必须看得出这条不是我写的）。
    ///
    /// **键位在这里归一**（Q2 / MR §6 留的欠账由 v2.186 兑现）：本方法的
    /// workspace 参数来自宿主自己解析的串，不像插件侧已过归一 ⇒ 不接就会造回
    /// 「同项目双键位」。归一走 `workspace::scope_key`（与注入键位、观测
    /// `scope_id`、`md_index_body` 的目录名同一个函数）；`md_write` 拿的是同一个
    /// 归一串 ⇒ sqlite 键位与人类层目录名同形。空串 = 全局层，照原样传下去。
    pub fn write(
        &self,
        workspace: &str,
        key: &str,
        value: &str,
        tags: &str,
        source: &str,
    ) -> Result<(), String> {
        if key.is_empty() || key.len() > KEY_MAX {
            return Err(format!("memory key 长度需在 1..={KEY_MAX} 字节"));
        }
        if value.len() > VALUE_MAX {
            return Err(format!("memory value 超过 {VALUE_MAX} 字节上限"));
        }
        let ws = crate::workspace::scope_key(Some(workspace));
        let conn = self.open_conn()?;
        let at = now_secs();
        conn.execute(
            "INSERT INTO memories (workspace, key, value, tags, updated_at, source) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(workspace, key) DO UPDATE SET
                 value = excluded.value, tags = excluded.tags,
                 updated_at = excluded.updated_at, source = excluded.source",
            params![ws, key, value, tags, at, source],
        )
        .map(|_| ())
        .map_err(|e| format!("memory write `{key}`: {e}"))?;
        self.md_write(&ws, key, value, tags, source, at);
        Ok(())
    }

    /// 管理页删除单条（按键位）。返回 false = 不存在。
    pub fn delete(&self, workspace: &str, key: &str) -> Result<bool, String> {
        let conn = self.open_conn()?;
        let n = conn
            .execute(
                "DELETE FROM memories WHERE workspace = ?1 AND key = ?2",
                params![workspace, key],
            )
            .map_err(|e| format!("memory delete `{key}`: {e}"))?;
        if n > 0 {
            self.md_delete(workspace, key);
        }
        Ok(n > 0)
    }

    /// 管理页清空（§11.3 一键清空，UI 只加一次确认；全键位）。返回条数。
    pub fn clear_all(&self) -> Result<usize, String> {
        let conn = self.open_conn()?;
        // F1：先收键位清单，删库后逐条同步删 md
        let keys: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare("SELECT workspace, key FROM memories")
                .map_err(|e| format!("memory clear: {e}"))?;
            let it = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(|e| format!("memory clear: {e}"))?;
            it.collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("memory clear: {e}"))?
        };
        let n = conn
            .execute("DELETE FROM memories", [])
            .map_err(|e| format!("memory clear: {e}"))?;
        for (ws, key) in keys {
            self.md_delete(&ws, &key);
        }
        Ok(n)
    }

    /// 首启导出全量行（F1；无管理页 200 条上限，出界防护 10 万行）。
    pub fn export_rows(&self) -> Result<Vec<MemoryRow>, String> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT workspace, key, value, tags, source, updated_at FROM memories
                 ORDER BY workspace, key LIMIT 100000",
            )
            .map_err(|e| format!("memory export: {e}"))?;
        let rows = stmt
            .query_map([], Self::row)
            .map_err(|e| format!("memory export: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("memory export: {e}"))?;
        Ok(rows)
    }

    // ---- F2 reconcile 专用（memory_reconcile.rs）：索引向文件对齐的
    // 直写 SQL——**绝不走** write/delete 常规路径（那会反向同步 md，覆盖
    // 人的文件）；上限校验同常规路径，超限 Err 由调用方计警告跳过。 ----

    /// 一个键位的全部行（reconcile 删除判定用；无上限——reconcile 是
    /// 全量对账，出界防护同 export）。
    pub fn reconcile_rows(&self, workspace: &str) -> Result<Vec<MemoryRow>, String> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT workspace, key, value, tags, source, updated_at FROM memories
                 WHERE workspace = ?1 ORDER BY key LIMIT 100000",
            )
            .map_err(|e| format!("memory reconcile rows: {e}"))?;
        let rows = stmt
            .query_map(params![workspace], Self::row)
            .map_err(|e| format!("memory reconcile rows: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("memory reconcile rows: {e}"))?;
        Ok(rows)
    }

    /// 以文件内容覆盖/新建一行（文件为准；tags/source 由调用方裁决）。
    pub fn reconcile_upsert(
        &self,
        workspace: &str,
        key: &str,
        value: &str,
        tags: &str,
        source: &str,
        at: i64,
    ) -> Result<(), String> {
        if key.is_empty() || key.len() > KEY_MAX {
            return Err(format!("memory key 长度需在 1..={KEY_MAX} 字节"));
        }
        if value.len() > VALUE_MAX {
            return Err(format!("memory value 超过 {VALUE_MAX} 字节上限"));
        }
        let conn = self.open_conn()?;
        conn.execute(
            "INSERT INTO memories (workspace, key, value, tags, updated_at, source) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(workspace, key) DO UPDATE SET
                 value = excluded.value, tags = excluded.tags,
                 updated_at = excluded.updated_at, source = excluded.source",
            params![workspace, key, value, tags, at, source],
        )
        .map(|_| ())
        .map_err(|e| format!("memory reconcile upsert `{key}`: {e}"))
    }

    /// 删除一行（人删了文件——单文件手势）。
    pub fn reconcile_delete(&self, workspace: &str, key: &str) -> Result<bool, String> {
        let conn = self.open_conn()?;
        let n = conn
            .execute(
                "DELETE FROM memories WHERE workspace = ?1 AND key = ?2",
                params![workspace, key],
            )
            .map_err(|e| format!("memory reconcile delete `{key}`: {e}"))?;
        Ok(n > 0)
    }

    /// 全部键位清单（reconcile 的 scope 枚举 ∪ fsRoots 作反查候选）。
    pub fn all_workspaces(&self) -> Result<Vec<String>, String> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT workspace FROM memories ORDER BY workspace")
            .map_err(|e| format!("memory workspaces: {e}"))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| format!("memory workspaces: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("memory workspaces: {e}"))?;
        Ok(rows)
    }

    fn recall_quiet(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryRow> {
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
        let Some(log) = &self.recall_log else {
            return;
        };
        let keys: Vec<String> = rows.iter().map(|r| r.key.clone()).collect();
        let scope = self.scope_of_session(session_id);
        // 无会话传空串而不是 NULL：部分唯一索引里 NULL ≠ NULL，用 NULL 会让
        // 「无会话的注入」每 step 插一条，去重直接失效（recall_log.rs 不变量 3）。
        if let Err(e) = log.note_inject(
            session_id.unwrap_or_default(),
            &scope,
            n_candidates,
            rows.len(),
            &keys,
            started.elapsed(),
        ) {
            tracing::warn!("recall observability: {e}");
        }
    }
}

/// 旧库迁移：无 workspace 列 = key 单主键时代，整表重建进未归属键位。
/// 与插件侧（memory-sqlite store.rs）等价幂等：BEGIN IMMEDIATE + 二次
/// 探针，两侧并发只有一方真正执行；旧表 source 列可有可无（headless
/// 时代插件建的库没有）——探针决定 SELECT 中的 source 表达式。
fn ensure_workspace_schema(conn: &Connection) -> Result<(), String> {
    let has_col = |name: &str| -> Result<i64, String> {
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('memories') WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .map_err(|e| format!("memory db schema probe: {e}"))
    };
    if has_col("workspace")? > 0 {
        return Ok(());
    }
    let source_expr = if has_col("source")? > 0 {
        "source"
    } else {
        "'explicit'"
    };
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| format!("memory db migrate begin: {e}"))?;
    let migrate = || -> Result<(), String> {
        if has_col("workspace")? > 0 {
            return Ok(());
        }
        conn.execute_batch(&format!(
            "ALTER TABLE memories RENAME TO memories_legacy_ws;
             {CREATE_TABLE}
             INSERT INTO memories (workspace, key, value, tags, updated_at, source)
                 SELECT '', key, value, tags, updated_at, {source_expr} FROM memories_legacy_ws;
             DROP TABLE memories_legacy_ws;"
        ))
        .map_err(|e| format!("memory db migrate to workspace key: {e}"))
    };
    match migrate() {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|e| format!("memory db migrate commit: {e}")),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}

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
    /// 未归属会话 = 没有"本项目"可言 ⇒ `None`（整节缺席）。两档数据的优先关系
    /// 在 `memory_index` 里：**md 投影优先**（人可编辑的人类层），md 用不上才由
    /// sqlite 现合成兜底（**不回写文件**）。取数失败 = `None` + 一次告警，
    /// 绝不合成一条"看起来是空索引"的块——那会把"读不到"骗成"没东西可注入"。
    fn project_index(&self, session_id: Option<&str>) -> Option<String> {
        let ws = self.scope_of_session(session_id);
        if ws.is_empty() {
            return None;
        }
        if let Some(block) = crate::memory_index::from_md(self.md_index_body(&ws).as_deref()) {
            return Some(block);
        }
        match self.index_rows(&ws, crate::memory_index::MAX_INDEX_LINES + 1) {
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
        }
    }
}

/// 组装点便利：Arc 化并作为 trait 对象交给 MemoryInject。
pub fn memory_source(store: &Arc<MemoryStore>) -> Arc<dyn MemorySource> {
    Arc::clone(store) as Arc<dyn MemorySource>
}

#[cfg(test)]
mod tests;
