//! 使用统计地基（⑩，exposure 文档 §3）：宿主自有 `<数据根>/usage.sqlite3`
//! （与 memory.sqlite3 分离——统计是宿主对插件的观测，不是插件数据）。
//!
//! 形状：tool_usage（调用计数 + ewma_score 预留列，⑫ EWMA 在此积累）/
//! plugin_usage（只有两个时间戳，§3.2"用户只对多久没用有概念"）。
//! 隐私（§3.6）：只记工具名、插件 id、时间戳——不记参数、结果、消息。
//!
//! 写入路径：runner 投影到 ToolResult 时 `record()` 进无界通道，后台任务
//! 聚合缓冲、每 5s 批量刷盘（§3.3，崩溃最多丢 5 秒）；所有句柄 drop 时
//! 收尾刷一次。库打不开 → 降级为丢弃（统计不值得为它中断聊天）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::idle::{classify, filter_key, IdleInfo};
use plugin_service::{InstallSource, InstalledPlugin};

/// 单工具聚合（5s 窗口内的增量）。
#[derive(Debug, Default, Clone)]
pub(crate) struct ToolAgg {
    plugin_id: Option<String>,
    calls: u64,
    ok: u64,
    last_ts: i64,
}

impl ToolAgg {
    /// 仅测试：跨模块组装聚合增量（字段私有）。
    #[cfg(test)]
    pub(crate) fn of(plugin_id: &str, calls: u64, ts: i64) -> ToolAgg {
        ToolAgg {
            plugin_id: Some(plugin_id.into()),
            calls,
            ok: calls,
            last_ts: ts,
        }
    }
}

/// sqlite 存取（同步、短事务；调用方负责不持锁跨 await）。
pub(crate) struct UsageDb {
    conn: Mutex<Connection>,
}

impl UsageDb {
    /// 开库建表（幂等）。文件损坏/不可写在此报错，由调用方降级。
    pub(crate) fn open(path: &Path) -> Result<UsageDb, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tool_usage (
                 tool_name     TEXT PRIMARY KEY,
                 plugin_id     TEXT NOT NULL,
                 call_count    INTEGER NOT NULL DEFAULT 0,
                 success_count INTEGER NOT NULL DEFAULT 0,
                 last_used_at  INTEGER NOT NULL,
                 ewma_score    REAL NOT NULL DEFAULT 0.0
             );
             CREATE INDEX IF NOT EXISTS idx_tool_usage_plugin ON tool_usage(plugin_id);
             CREATE TABLE IF NOT EXISTS plugin_usage (
                 plugin_id     TEXT PRIMARY KEY,
                 first_used_at INTEGER NOT NULL,
                 last_used_at  INTEGER NOT NULL
             );",
        )
        .map_err(|e| e.to_string())?;
        Ok(UsageDb {
            conn: Mutex::new(conn),
        })
    }

    /// 单条直写（测试用；生产路径走 flush 聚合）。
    #[cfg(test)]
    fn record(&self, tool: &str, plugin_id: &str, ok: bool, ts: i64) {
        let mut tools = HashMap::new();
        tools.insert(
            tool.to_string(),
            ToolAgg {
                plugin_id: Some(plugin_id.to_string()),
                calls: 1,
                ok: u64::from(ok),
                last_ts: ts,
            },
        );
        let mut plugins = HashMap::new();
        plugins.insert(plugin_id.to_string(), ts);
        self.flush(&tools, &plugins).expect("direct record");
    }

    /// 批量刷盘：单事务 upsert。plugin_usage 冲突时只更新 last_used_at，
    /// first_used_at 保留首用值。
    pub(crate) fn flush(
        &self,
        tools: &HashMap<String, ToolAgg>,
        plugins: &HashMap<String, i64>,
    ) -> Result<(), String> {
        if tools.is_empty() && plugins.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().expect("usage db poisoned");
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for (name, agg) in tools {
            // ⑫ EWMA 积累：读旧分与旧时刻，衰减后加本窗口调用数。
            // 新行无旧值 → (0.0, 本窗口时刻)（decay=1，纯加法）。
            let (old_score, old_last): (f64, i64) = tx
                .query_row(
                    "SELECT ewma_score, last_used_at FROM tool_usage WHERE tool_name = ?1",
                    [name.as_str()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or((0.0, agg.last_ts));
            let score = update_ewma(old_score, agg.last_ts, old_last, agg.calls);
            tx.execute(
                "INSERT INTO tool_usage (tool_name, plugin_id, call_count, success_count, last_used_at, ewma_score)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(tool_name) DO UPDATE SET
                     call_count = call_count + excluded.call_count,
                     success_count = success_count + excluded.success_count,
                     last_used_at = excluded.last_used_at,
                     ewma_score = excluded.ewma_score",
                rusqlite::params![name, agg.plugin_id.as_deref().unwrap_or(""), agg.calls as i64, agg.ok as i64, agg.last_ts, score],
            )
            .map_err(|e| format!("tool_usage upsert {name}: {e}"))?;
        }
        for (id, ts) in plugins {
            tx.execute(
                "INSERT INTO plugin_usage (plugin_id, first_used_at, last_used_at)
                 VALUES (?1, ?2, ?2)
                 ON CONFLICT(plugin_id) DO UPDATE SET last_used_at = excluded.last_used_at",
                rusqlite::params![id, ts],
            )
            .map_err(|e| format!("plugin_usage upsert {id}: {e}"))?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// 全表插件时间戳（plugin_list_with_usage 投影用）。
    pub(crate) fn plugin_times(&self) -> Result<HashMap<String, (i64, i64)>, String> {
        let conn = self.conn.lock().expect("usage db poisoned");
        let mut stmt = conn
            .prepare("SELECT plugin_id, first_used_at, last_used_at FROM plugin_usage")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        for row in rows {
            let (id, first, last) = row.map_err(|e| e.to_string())?;
            map.insert(id, (first, last));
        }
        Ok(map)
    }

    /// 调用次数最高的工具（⑫ search_tools 空结果回退 top-10 高频，§13）。
    pub(crate) fn hot_tools(&self, limit: u32) -> Result<Vec<(String, u64)>, String> {
        let conn = self.conn.lock().expect("usage db poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT tool_name, call_count FROM tool_usage
                 ORDER BY call_count DESC, tool_name ASC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// 全表 EWMA 分（⑫ ToolExposure 打分用）。
    pub(crate) fn ewma_scores(&self) -> Result<HashMap<String, f64>, String> {
        let conn = self.conn.lock().expect("usage db poisoned");
        let mut stmt = conn
            .prepare("SELECT tool_name, ewma_score FROM tool_usage")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
            .map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        for row in rows {
            let (name, score) = row.map_err(|e| e.to_string())?;
            map.insert(name, score);
        }
        Ok(map)
    }
}

struct UsageRec {
    tool: String,
    plugin_id: Option<String>,
    ok: bool,
    ts: i64,
}

/// 5s 批量刷盘的写入端（Clone 不需要：AppState 持 Arc）。
pub struct UsageRecorder {
    tx: Option<mpsc::UnboundedSender<UsageRec>>,
    db: Option<Arc<UsageDb>>,
}

impl UsageRecorder {
    /// 起后台聚合任务。所有 UsageRecorder drop 后通道关闭，任务收尾刷盘。
    pub fn spawn(db: UsageDb) -> Arc<UsageRecorder> {
        let db = Arc::new(db);
        let (tx, mut rx) = mpsc::unbounded_channel::<UsageRec>();
        let task_db = Arc::clone(&db);
        tauri::async_runtime::spawn(async move {
            let mut tools: HashMap<String, ToolAgg> = HashMap::new();
            let mut plugins: HashMap<String, i64> = HashMap::new();
            loop {
                // 5s 无新事件即刷盘（§3.3 崩溃最多丢 5 秒）
                match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
                    Ok(Some(rec)) => {
                        let agg = tools.entry(rec.tool).or_default();
                        if agg.plugin_id.is_none() {
                            agg.plugin_id = rec.plugin_id.clone();
                        }
                        agg.calls += 1;
                        agg.ok += u64::from(rec.ok);
                        agg.last_ts = rec.ts;
                        if let Some(pid) = rec.plugin_id {
                            plugins.insert(pid, rec.ts);
                        }
                    }
                    Ok(None) => break, // 全部句柄 drop → 收尾
                    Err(_) => {}       // 5s 到点 → 刷盘
                }
                if let Err(e) = task_db.flush(&tools, &plugins) {
                    tracing::warn!("usage flush: {e}");
                }
                tools.clear();
                plugins.clear();
            }
            let _ = task_db.flush(&tools, &plugins);
        });
        Arc::new(UsageRecorder {
            tx: Some(tx),
            db: Some(db),
        })
    }

    /// 降级模式：库不可用，记录全部丢弃（§3.3 语义之外的兜底）。
    pub fn disabled() -> Arc<UsageRecorder> {
        Arc::new(UsageRecorder { tx: None, db: None })
    }

    /// 仅测试：不 spawn 后台任务的直连构造（查询方法可用，写入通道关闭）。
    #[cfg(test)]
    pub(crate) fn direct(db: UsageDb) -> Arc<UsageRecorder> {
        Arc::new(UsageRecorder {
            tx: None,
            db: Some(Arc::new(db)),
        })
    }

    /// ToolResult 后异步更新（exposure 文档 §3.3）：只记名字与结果成败。
    pub fn record(&self, tool: &str, plugin_id: Option<&str>, ok: bool) {
        let Some(tx) = &self.tx else { return };
        let ts = now_secs();
        let _ = tx.send(UsageRec {
            tool: tool.to_string(),
            plugin_id: plugin_id.map(str::to_owned),
            ok,
            ts,
        });
    }

    /// 插件时间戳查询（投影用）；降级模式返回空表。
    pub fn plugin_times(&self) -> HashMap<String, (i64, i64)> {
        self.db
            .as_ref()
            .and_then(|db| match db.plugin_times() {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!("usage query: {e}");
                    None
                }
            })
            .unwrap_or_default()
    }

    /// 调用次数 top-N（⑫ search_tools 回退用）；降级模式返回空表。
    pub fn hot_tools(&self, limit: u32) -> Vec<(String, u64)> {
        self.db
            .as_ref()
            .and_then(|db| match db.hot_tools(limit) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("usage hot_tools: {e}");
                    None
                }
            })
            .unwrap_or_default()
    }

    /// 全表 EWMA 分（⑫ ToolExposure 打分用）；降级模式返回空表。
    pub fn ewma_scores(&self) -> HashMap<String, f64> {
        self.db
            .as_ref()
            .and_then(|db| match db.ewma_scores() {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!("usage ewma: {e}");
                    None
                }
            })
            .unwrap_or_default()
    }
}

/// plugin_list_with_usage 的行（⑩⑤）：InstalledPlugin 按 id join 使用
/// 统计后的增量视图，前端拿它合并既有列表。builtin 开发态插件不评估
/// 闲置（installed_at 无 lockfile 真源，标"多久没用"是误导）。
#[derive(Debug, Clone, Serialize)]
pub struct PluginUsageView {
    pub id: String,
    pub enabled: bool,
    pub active: bool,
    pub installed_at: i64,
    pub first_used_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub idle: Option<IdleInfo>,
    pub filter_key: Option<&'static str>,
}

/// 已安装列表 × 使用统计 → 投影（exposure 文档 §10.1：前端不做时间计算）。
pub fn attach(
    plugins: &[InstalledPlugin],
    now: i64,
    times: &HashMap<String, (i64, i64)>,
) -> Vec<PluginUsageView> {
    plugins
        .iter()
        .map(|p| {
            let row = times.get(&p.id).copied();
            let (first_used_at, last_used_at) = row
                .map(|(first, last)| (Some(first), Some(last)))
                .unwrap_or((None, None));
            let builtin = matches!(p.source, InstallSource::Builtin);
            let idle = if builtin {
                None
            } else {
                Some(classify(now, p.installed_at, last_used_at))
            };
            let filter_key = idle.as_ref().map(filter_key);
            PluginUsageView {
                id: p.id.clone(),
                enabled: p.enabled,
                active: p.active_version.is_some(),
                installed_at: p.installed_at,
                first_used_at,
                last_used_at,
                idle,
                filter_key,
            }
        })
        .collect()
}

mod ewma;

use crate::timeutil::now_secs;
use ewma::update_ewma;
pub use ewma::UsageEwma;

#[cfg(test)]
mod tests;
