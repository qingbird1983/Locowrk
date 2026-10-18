//! **导出与对账直写面**（台账 T33）：`export_rows`（F1 首启导出全量行）加上
//! `memory_reconcile.rs` 专用的四条——`reconcile_rows` / `reconcile_upsert` /
//! `reconcile_delete` / `all_workspaces`。
//!
//! 这一面**绝不走** `write` / `delete` 常规路径（那会反向同步 md，覆盖人的文件），
//! 所以 SQL 各写一份是刻意的；那条纪律原文就写在这段中间的 `----` 注释里，跟着搬过来了。
//! 出界防护恒 10 万行（不像管理页有 `LIST_MAX`）：对账是全量对账。
//!
//! 首尾两行是本片自补的语法外壳（同 `crud.rs`）。

use rusqlite::params;

use super::shapes::{MemoryRow, KEY_MAX, VALUE_MAX};
use super::store::MemoryStore;

impl MemoryStore {
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
}
