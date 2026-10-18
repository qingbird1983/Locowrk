//! 记忆库的**常规读写面**（台账 T33）：管理页列举 `list`、编辑 `update_value`、
//! 写入 `write`（生产写路径三处共用这条 upsert）、删除 `delete`、清空 `clear_all`。
//! 五条全部走 `store.rs` 那一个 `open_conn`，键位归一与上限校验留在各自主体里。
//!
//! ⚠ 登记不合并：`key`/`value` 上限校验这一句话在本片出现两次、在 `reconcile.rs` 出现
//! 一次（三个出处），memories 的 upsert 同理两处——那是台账 T28 同族的欠账，本刀只搬不改，
//! 因为"搬动"和"改语义"混进一份 diff 就没法用行守恒判据了。
//!
//! 首尾两行（`impl MemoryStore {` 与闭合）是本片自补的语法外壳，不是搬来的内容；
//! 字段与方法经同一棵模块树可见 ⇒ 本片不新增任何抬升。

use rusqlite::params;

use crate::timeutil::now_secs;

use super::shapes::{MemoryRow, KEY_MAX, LIST_MAX, VALUE_MAX};
use super::store::MemoryStore;

impl MemoryStore {
    /// 管理页列举（全键位）：query 空 = 全量（新→旧），否则 key/value/tags
    /// 子串过滤（LIKE 通配符按字面量，与 memory_search 同纪律）。
    pub fn list(&self, query: &str, limit: u32) -> Result<Vec<MemoryRow>, String> {
        let limit = limit.clamp(1, LIST_MAX);
        // 刀②b-1：人类层可用 ⇒ **读源就是 md**，一条 SQL 都不发。
        // 还能落到下面那条 SQL 通路的只剩"未接线（单测）/ 总开关关"这一档，
        // 它按刀②c 取消；现在留着是为了让既有那些 `MemoryStore::new()` 的用例
        // 一字未改仍然绿（否则这一刀的 diff 里就分不清"行为变了"还是"夹具变了"）。
        if let Some(files) = self.files_face() {
            return Ok(Self::md_rows(files.root(), query, limit as usize));
        }
        let conn = self.open_conn()?;
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

    /// 人类层列举（刀②b-1 的读源）。三条口径与上面那条 SQL 通路**逐字对齐**，
    /// 因为"换读源"不该顺手改掉管理页的行为：
    /// ① 排序 = `updated_at DESC, key`（`memlayout::iter_facts` 里已按这条排，
    ///    对齐的理由与后果写在那片头注）；
    /// ② 过滤 = key / value / tags 任一子串命中。SQLite 的 `LIKE` 默认对 **ASCII
    ///    大小写不敏感** ⇒ 这里必须折叠大小写：写成裸 `contains` 就变成"搜 rust
    ///    搜不到 Rust"那种没人会立刻发现的小退化（`%` `_` 在 SQL 侧被 ESCAPE 成
    ///    字面量，在 `contains` 侧天然是字面量 ⇒ 这一维不用做转义）；
    /// ③ 条数上限 = 同一个钳过的 `limit`。
    /// `unreadable` 只 warn、不报错也不静默：列表照常出其余那些，但"扫不动几条"
    /// 必须落到日志里——它记的是"没给看的条数"，不是"库里没有"。
    fn md_rows(root: &std::path::Path, query: &str, limit: usize) -> Vec<MemoryRow> {
        let scan = plugin_api::memlayout::iter_facts(root);
        if scan.unreadable > 0 {
            tracing::warn!(
                "记忆 md 列举：{} 条读不动（未计进本次列表）",
                scan.unreadable
            );
        }
        let needle = query.to_ascii_lowercase();
        let hit = |s: &str| needle.is_empty() || s.to_ascii_lowercase().contains(&needle);
        scan.facts
            .iter()
            .filter(|f| hit(&f.key) || hit(&f.value) || hit(&f.tags))
            .take(limit)
            .map(MemoryRow::from)
            .collect()
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
}
