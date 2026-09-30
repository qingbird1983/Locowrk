//! 记忆库宿主侧访问（⑪）：`<数据根>/memory.sqlite3` 归 memory-sqlite
//! 插件所有（schema 权威在其 store.rs）；本模块做两类宿主自有访问——
//! ① `MemorySource` 实现：Memory-Inject（kernel-agent）每 step 的检索源。
//!    不走工具管道（否则每 step 一次 memory_list 会污染使用统计与审计），
//!    直读同一 sqlite 文件（两侧都是短连接，busy_timeout 兜底并发）。
//! ② 记忆管理页的存储支撑（Context and Memory.md §7.2）：列举 / 编辑 /
//!    删除 / 清空——Tauri 命令薄适配到这里，命令层不写 SQL。
//!
//! source 列（explicit | extracted，§4.5 管理页要显示来源）归宿主侧
//! 演进：建表即带；旧库（插件早期建的表）用 pragma 探测后 ALTER 补列，
//! DEFAULT 'explicit' 覆盖既有行——插件 INSERT 不指定该列，自然填充。
//!
//! 降级纪律：打不开 = 检索降级空表（仅告警一次，注入不拦聊天）+
//! 管理命令报错给 UI；不在启动路径上 panic。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rusqlite::{Connection, params};
use serde::Serialize;

use kernel_agent::{MemoryEntry, MemorySource};

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
    pub updated_at: i64,
}

pub struct MemoryStore {
    dir: PathBuf,
    /// 检索失败只告警一次（每 step 都 recall，别刷日志）。
    warned: std::sync::atomic::AtomicBool,
}

impl MemoryStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            warned: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// 按调用开连接、用完即弃（与插件同款）；建表 + 旧库补列幂等。
    fn open_conn(&self) -> Result<Connection, String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("memory db dir `{}`: {e}", self.dir.display()))?;
        let conn = Connection::open(self.dir.join("memory.sqlite3"))
            .map_err(|e| format!("memory db open: {e}"))?;
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(|e| format!("memory db busy_timeout: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                 key        TEXT PRIMARY KEY,
                 value      TEXT NOT NULL,
                 tags       TEXT NOT NULL DEFAULT '',
                 updated_at INTEGER NOT NULL,
                 source     TEXT NOT NULL DEFAULT 'explicit'
             );",
        )
        .map_err(|e| format!("memory db init: {e}"))?;
        let has_source: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('memories') WHERE name = 'source'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("memory db schema probe: {e}"))?;
        if has_source == 0 {
            conn.execute_batch(
                "ALTER TABLE memories ADD COLUMN source TEXT NOT NULL DEFAULT 'explicit';",
            )
            .map_err(|e| format!("memory db migrate source column: {e}"))?;
        }
        Ok(conn)
    }

    fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRow> {
        Ok(MemoryRow {
            key: r.get(0)?,
            value: r.get(1)?,
            tags: r.get(2)?,
            source: r.get(3)?,
            updated_at: r.get(4)?,
        })
    }

    /// 管理页列举：query 空 = 全量（新→旧），否则 key/value/tags 子串过滤
    /// （LIKE 通配符按字面量，与 memory_search 同纪律）。
    pub fn list(&self, query: &str, limit: u32) -> Result<Vec<MemoryRow>, String> {
        let conn = self.open_conn()?;
        let limit = limit.clamp(1, LIST_MAX);
        if query.is_empty() {
            let mut stmt = conn
                .prepare(
                    "SELECT key, value, tags, source, updated_at FROM memories
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
            query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
        );
        let mut stmt = conn
            .prepare(
                "SELECT key, value, tags, source, updated_at FROM memories
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
    /// 返回 false = 键不存在。
    pub fn update_value(&self, key: &str, value: &str) -> Result<bool, String> {
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
                "UPDATE memories SET value = ?2, updated_at = ?3 WHERE key = ?1",
                params![key, value, now],
            )
            .map_err(|e| format!("memory update `{key}`: {e}"))?;
        Ok(n > 0)
    }

    /// 管理页删除单条。返回 false = 键不存在。
    pub fn delete(&self, key: &str) -> Result<bool, String> {
        let conn = self.open_conn()?;
        let n = conn
            .execute("DELETE FROM memories WHERE key = ?1", params![key])
            .map_err(|e| format!("memory delete `{key}`: {e}"))?;
        Ok(n > 0)
    }

    /// 管理页清空（§11.3 一键清空，UI 只加一次确认）。返回删除条数。
    pub fn clear_all(&self) -> Result<usize, String> {
        let conn = self.open_conn()?;
        conn.execute("DELETE FROM memories", [])
            .map_err(|e| format!("memory clear: {e}"))
    }

    fn recall_quiet(&self, limit: usize) -> Vec<MemoryRow> {
        match self.list("", (limit as u32).max(1)) {
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

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Memory-Inject 的检索源（注册进 agent/request 决策环）。
impl MemorySource for MemoryStore {
    fn recall(&self, limit: usize) -> Vec<MemoryEntry> {
        self.recall_quiet(limit)
            .into_iter()
            .map(|r| MemoryEntry {
                key: r.key,
                value: r.value,
                tags: r.tags,
                updated_at: r.updated_at,
            })
            .collect()
    }
}

/// 组装点便利：Arc 化并作为 trait 对象交给 MemoryInject。
pub fn memory_source(store: &Arc<MemoryStore>) -> Arc<dyn MemorySource> {
    Arc::clone(store) as Arc<dyn MemorySource>
}

#[cfg(test)]
mod tests;
