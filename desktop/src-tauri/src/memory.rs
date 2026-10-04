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
//! **项目隔离 + 双层记忆（Context 文档 §4.6，㉘·A·fix）**：注入召回 =
//! 会话归属键位 ∪ 全局层（`''` = 用户喜好偏好，对一切会话可见；未归属
//! 会话只落在全局层）；管理页是人的管理面，看全部并透出归属。降级纪律：
//! 打不开 = 检索降级空表（仅告警一次，注入不拦聊天）+ 管理命令报错给
//! UI；不在启动路径上 panic。

use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;

use kernel_agent::{MemoryEntry, MemorySource};

use crate::workspace::{scope_key, WorkspaceResolver};

/// 与 memory-sqlite 插件同限：key 主键不能无限长，value 防 runaway 写。
const KEY_MAX: usize = 512;
const VALUE_MAX: usize = 256 * 1024;
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
    /// F1 人类层（md 双写，文件化设计 §3）：sqlite 为真源，四条宿主写
    /// 路径（提取 write / 管理页 update·delete·clear）成功后同步 md；
    /// 未接线（单测）= 纯 sqlite。
    files: Option<Arc<crate::memory_files::MemoryFiles>>,
    /// 文件化总开关（settings `memory.files` 闭包；缺省无接线 = 恒关）。
    files_on: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl MemoryStore {
    pub fn new(dir: &Path, workspace_of: WorkspaceResolver) -> Self {
        Self {
            dir: dir.to_path_buf(),
            workspace_of,
            warned: AtomicBool::new(false),
            files: None,
            files_on: Arc::new(|| false),
        }
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

    /// 会话（可缺）→ 键位（未归属 = ""）。extract 与 MemorySource 共用。
    pub fn scope_of_session(&self, session_id: Option<&str>) -> String {
        scope_key(session_id.and_then(|s| (self.workspace_of)(s)).as_deref())
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

    /// 写入/覆盖一条记忆（⑬ 隐式提取的落库路径；管理面不用它）。键位 =
    /// (workspace, key)——调用方负责命名空间（extract 侧强制 `auto.` 前缀，
    /// 永不覆盖用户显式记忆）；source = "explicit" | "extracted"。
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
        let conn = self.open_conn()?;
        let at = now_secs();
        conn.execute(
            "INSERT INTO memories (workspace, key, value, tags, updated_at, source) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(workspace, key) DO UPDATE SET
                 value = excluded.value, tags = excluded.tags,
                 updated_at = excluded.updated_at, source = excluded.source",
            params![workspace, key, value, tags, at, source],
        )
        .map(|_| ())
        .map_err(|e| format!("memory write `{key}`: {e}"))?;
        self.md_write(workspace, key, value, tags, source, at);
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

    fn recall_quiet(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryRow> {
        let ws = self.scope_of_session(session_id);
        // 双层记忆（Context 文档 §4.6，㉘·A·fix）：项目键位 ∪ 全局层（''，
        // 用户喜好偏好）——归属会话两层都见；未归属会话只落在全局层
        // （?1 = '' 时两条件重合），任何别的项目行不可见。
        let recalled: Result<Vec<MemoryRow>, String> = (|| {
            let conn = self.open_conn()?;
            let mut stmt = conn
                .prepare(
                    "SELECT workspace, key, value, tags, source, updated_at FROM memories
                     WHERE workspace = ?1 OR workspace = ''
                     ORDER BY updated_at DESC, key LIMIT ?2",
                )
                .map_err(|e| format!("memory recall: {e}"))?;
            let rows = stmt
                .query_map(params![ws, limit as i64], Self::row)
                .map_err(|e| format!("memory recall: {e}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("memory recall: {e}"))?;
            Ok(rows)
        })();
        match recalled {
            Ok(rows) => rows,
            Err(e) => {
                if !self.warned.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    tracing::warn!("memory recall unavailable（记忆注入降级为空）: {e}");
                }
                Vec::new()
            }
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

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Memory-Inject 的检索源（注册进 agent/request 决策环）。㉘：按会话
/// 归属的项目键位 ∪ 全局层过滤（workspace.rs 解析；未归属只见全局层）。
/// ㉘·A·fix 另供两件：`scope_id`（预取缓存键成分——归属变更自动重算）
/// 与 `context_line`（归属会话告知模型当前项目根，"看下这个项目"不再
/// 靠旧记忆猜路径）。
impl MemorySource for MemoryStore {
    fn recall(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry> {
        self.recall_quiet(session_id, limit)
            .into_iter()
            .map(|r| MemoryEntry {
                key: r.key,
                value: r.value,
                tags: r.tags,
                updated_at: r.updated_at,
                scope: if r.workspace.is_empty() {
                    "global".into()
                } else {
                    "project".into()
                },
            })
            .collect()
    }

    fn scope_id(&self, session_id: Option<&str>) -> String {
        self.scope_of_session(session_id)
    }

    fn context_line(&self, session_id: Option<&str>) -> Option<String> {
        let ws = self.scope_of_session(session_id);
        if ws.is_empty() {
            return None;
        }
        Some(format!(
            "Current project workspace: {ws} — \"this project\" in the user's \
             message refers to this directory; list/read it first, do not guess \
             other paths from old memories."
        ))
    }
}

/// 组装点便利：Arc 化并作为 trait 对象交给 MemoryInject。
pub fn memory_source(store: &Arc<MemoryStore>) -> Arc<dyn MemorySource> {
    Arc::clone(store) as Arc<dyn MemorySource>
}

#[cfg(test)]
mod tests;
