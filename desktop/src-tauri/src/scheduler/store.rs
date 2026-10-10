//! 调度存储（⑭）：宿主自有 `<数据根>/schedule.sqlite3`（usage / memory
//! 两库同款"一关切一库"先例——调度是宿主对时间的主张，不属
//! 任何插件）。同步、短事务；调用方不持锁跨 await。
//!
//! 表语义：调度 = 待触发的注入轮。一次性触发后即删（"证据在会话日志"，
//! 不留已完成的行）；周期触发原地改 `run_at`。`deferrals` 记会话忙的
//! 顺延次数（忙 ≠ 丢，见 engine 的让位上限）。

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

/// 防失控上限：模型反复 create 打满即拒（错误如实回给模型）。
pub(crate) const MAX_SCHEDULES: i64 = 100;

/// 顺延步长与上限（engine 也用）：会话忙 30s 后重试；一次性累计顺延
/// 超过 2 小时（240 × 30s）放弃并通知，周期让出一个槽位不追帧。
pub(crate) const DEFER_STEP_MS: i64 = 30_000;
pub(crate) const ONCE_MAX_DEFERRALS: i64 = 240;
pub(crate) const INTERVAL_MAX_DEFERRALS: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Once,
    Interval,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Once => "once",
            Kind::Interval => "interval",
        }
    }

    fn from_db(s: &str) -> Kind {
        if s == "interval" {
            Kind::Interval
        } else {
            Kind::Once
        }
    }
}

/// 一条待触发的调度（行 ⇔ 结构体一一对应；id 由工具层生成 UUID 形态）。
#[derive(Debug, Clone)]
pub(crate) struct Schedule {
    pub id: String,
    pub session_id: String,
    pub kind: Kind,
    /// 通知/系统消息用短标题（创建时未给则工具层截 prompt 前缀）。
    pub title: String,
    /// 触发时注入目标会话的轮内容（模型原文，不再加工）。
    pub prompt: String,
    /// 下次触发时刻（unix 毫秒；due = run_at <= now）。
    pub run_at: i64,
    /// 周期间隔（秒）；一次性为 None。
    pub interval_secs: Option<i64>,
    pub deferrals: i64,
    /// 创建时刻（unix 毫秒，v2.57 目的地页排序/展示用）。
    pub created_ms: i64,
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) struct ScheduleDb {
    conn: Mutex<Connection>,
}

impl ScheduleDb {
    /// 开库建表（幂等）。文件损坏/不可写在此报错，由组装点降级
    /// （调度缺席：工具报错、引擎不起——聊天不受影响）。
    pub(crate) fn open(path: &Path) -> Result<ScheduleDb, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schedules (
                 id            TEXT PRIMARY KEY,
                 session_id    TEXT NOT NULL,
                 kind          TEXT NOT NULL CHECK (kind IN ('once','interval')),
                 title         TEXT NOT NULL,
                 prompt        TEXT NOT NULL,
                 run_at        INTEGER NOT NULL,
                 interval_secs INTEGER,
                 deferrals     INTEGER NOT NULL DEFAULT 0,
                 created_at    INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_schedules_run_at ON schedules(run_at);",
        )
        .map_err(|e| e.to_string())?;
        Ok(ScheduleDb {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn create(&self, s: &Schedule) -> Result<(), String> {
        let conn = self.conn.lock().expect("schedules lock poisoned");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM schedules", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if n >= MAX_SCHEDULES {
            return Err(format!(
                "schedule table full ({MAX_SCHEDULES}); delete some schedules first"
            ));
        }
        conn.execute(
            "INSERT INTO schedules
                 (id, session_id, kind, title, prompt, run_at, interval_secs, deferrals, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                s.id,
                s.session_id,
                s.kind.as_str(),
                s.title,
                s.prompt,
                s.run_at,
                s.interval_secs,
                s.deferrals,
                s.created_ms,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 某会话的待触发调度（schedule_list 用；新→旧按创建时间）。
    pub(crate) fn list_session(&self, session_id: &str) -> Vec<Schedule> {
        self.query(
            "SELECT id, session_id, kind, title, prompt, run_at, interval_secs, deferrals, created_at
             FROM schedules WHERE session_id = ?1 ORDER BY created_at DESC, id",
            rusqlite::params![session_id],
        )
    }

    /// 全部待触发行，新→旧按创建时间（v2.57 目的地页 `schedule_list` 用；
    /// 已触发的行不留库——一次性触发即删，证据在会话日志）。
    pub(crate) fn list_all(&self) -> Vec<Schedule> {
        self.query(
            "SELECT id, session_id, kind, title, prompt, run_at, interval_secs, deferrals, created_at
             FROM schedules ORDER BY created_at DESC, id",
            [],
        )
    }

    /// 全部到期（引擎 tick 与启动补跑共用一条路径——"应用未运行错过的"
    /// 就是启动后第一次 tick 的 run_at <= now）。
    pub(crate) fn due(&self, now_ms: i64) -> Vec<Schedule> {
        self.query(
            "SELECT id, session_id, kind, title, prompt, run_at, interval_secs, deferrals, created_at
             FROM schedules WHERE run_at <= ?1 ORDER BY run_at, id",
            rusqlite::params![now_ms],
        )
    }

    /// 原地改排程（顺延 / 周期滚到下槽）。
    pub(crate) fn reschedule(&self, id: &str, run_at: i64, deferrals: i64) {
        if let Err(e) = self.conn.lock().expect("schedules lock poisoned").execute(
            "UPDATE schedules SET run_at = ?2, deferrals = ?3 WHERE id = ?1",
            rusqlite::params![id, run_at, deferrals],
        ) {
            tracing::warn!("schedule {id} reschedule failed: {e}");
        }
    }

    /// 删除（一次性触发后 / schedule_delete / 放弃）。返回是否确有此行。
    pub(crate) fn delete(&self, id: &str) -> bool {
        self.conn
            .lock()
            .expect("schedules lock poisoned")
            .execute("DELETE FROM schedules WHERE id = ?1", rusqlite::params![id])
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    fn query(&self, sql: &str, params: impl rusqlite::Params) -> Vec<Schedule> {
        let conn = self.conn.lock().expect("schedules lock poisoned");
        let mut stmt = match conn.prepare(sql) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("schedules query failed: {e}");
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params, |r| {
            let kind: String = r.get("kind")?;
            let interval: Option<i64> = r.get("interval_secs")?;
            Ok(Schedule {
                id: r.get("id")?,
                session_id: r.get("session_id")?,
                kind: Kind::from_db(&kind),
                title: r.get("title")?,
                prompt: r.get("prompt")?,
                run_at: r.get("run_at")?,
                interval_secs: interval,
                deferrals: r.get("deferrals")?,
                created_ms: r.get("created_at")?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(e) => {
                tracing::warn!("schedules query failed: {e}");
                Vec::new()
            }
        }
    }
}
