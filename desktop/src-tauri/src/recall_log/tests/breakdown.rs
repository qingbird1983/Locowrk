//! 队列 Q10（L4）落盘面：`memory_inject_log.breakdown` 那一列的**三档语义**。
//!
//! 判的是"库里有哪一行带着什么"，不是分数怎么算（那边在 `memory_learn/tests/receipt.rs`）：
//! ①检索档没有加权这一步 ⇒ NULL；②注入档但学习档关着（走现役 SQL 本体）⇒ NULL；
//! ③注入档且开着 ⇒ 那一次的 JSON 回执。三档必须能被区分开，所以列不给
//! `NOT NULL DEFAULT ''`——空串会把①②读成"有回执但回执为空"，那是假数据。
//! 外加这条列自己带来的两个新面：**去重时先到先得**（不许后写覆盖第一次的回执）、
//! **存量库补列**（`CREATE TABLE IF NOT EXISTS` 对已存在的表不补列）。

use super::*;
use kernel_agent::{LearnWeights, MemorySource, RecallQuery};
use rusqlite::Connection;

fn learn(on: bool) -> RecallQuery {
    RecallQuery {
        terms: vec!["docker".to_string()],
        weights: LearnWeights {
            enabled: on,
            ..LearnWeights::default()
        },
    }
}

fn inject_row(log: &RecallLog, session: &str) -> RecallRow {
    rows(log)
        .into_iter()
        .find(|r| r.mode == "inject" && r.session_key == session)
        .unwrap_or_else(|| panic!("注入档 {session} 没有行 = 本条判据空转"))
}

/// ③开档注入：交付的每条都在回执里，回执带权重四档，且**不带走记忆正文**。
#[test]
fn a_learned_injection_writes_the_receipt_of_that_one_pass() {
    let (store, log) = store_with_log(
        "learned",
        &[
            ("", "deploy", "部署用 docker compose"),
            ("", "image", "docker 镜像走内网源"),
        ],
    );
    let got = store.recall_relevant(Some("ws-session"), 5, &learn(true));
    assert!(!got.is_empty(), "夹具没被交付 = 本条判据空转");
    let row = inject_row(&log, "ws-session");
    let json = row.breakdown.clone().expect("开档 = 有回执");
    let v: serde_json::Value = serde_json::from_str(&json).expect("回执要读得回来");
    let keys: Vec<&str> = v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["key"].as_str().unwrap())
        .collect();
    for e in &got {
        assert!(
            keys.contains(&e.key.as_str()),
            "`{}` 交付了却不在回执里 = 解释与实际交付脱钩",
            e.key
        );
    }
    assert_eq!(
        v["weights"].as_array().unwrap().len(),
        4,
        "权重四档要在回执里"
    );
    assert!(
        !json.contains("内网源"),
        "观测面只带键名与分数，正文一个字节都不抄"
    );
}

/// ②关档注入 = 走现役那条 SQL 本体，那一步压根没算分 ⇒ 必须是 NULL 而不是空串。
/// 与①同形不同因：①是"这一档不参与加权"，②是"这一档现在走的是老路径"。
#[test]
fn a_current_sql_injection_writes_no_receipt() {
    let (store, log) = store_with_log("off", &[("", "deploy", "部署用 docker")]);
    let got = store.recall_relevant(Some("ws-session"), 5, &learn(false));
    assert_eq!(got.len(), 1, "关档也要照常交付，本条判的不是交付");
    assert_eq!(
        inject_row(&log, "ws-session").breakdown,
        None,
        "关档写空串 = 把「没算分」冒充成「算了但没结果」"
    );
    // `MemorySource::recall`（无 query 的那条通路，内部走 `recall_quiet`）同样属于
    // ②这一档：它本来就是现役 SQL 本体。
    store.recall(Some("plain-session"), 5);
    assert_eq!(inject_row(&log, "plain-session").breakdown, None);
}

/// ①检索档：`note_receipt` 那条通路没有加权这一步，恒 NULL。判据盯着这条不许
/// 被后来的批拿"顺手补个分数"填上——那才是真造假。
#[test]
fn search_receipts_carry_nothing_in_the_receipt_column() {
    let (_store, log) = store_with_log("receipt", &[("", "k", "v")]);
    log.note_receipt(
        "memory_search",
        "ws-session",
        "F:/ws/a",
        1,
        &["k".to_string()],
        std::time::Duration::from_micros(7),
    )
    .unwrap();
    let r = rows(&log)
        .into_iter()
        .find(|x| x.mode == "memory_search")
        .expect("检索档要有一行");
    assert_eq!(r.breakdown, None);
}

/// 去重口径的新面：同键位第二次注入被 `INSERT OR IGNORE` 忽略 ⇒ **保留第一次那次的
/// 回执**，不是"最新一次覆盖"。这条如果漂了，面板显示的就不是用户看到的那一次。
#[test]
fn the_first_injection_of_a_keyside_keeps_its_receipt() {
    let (store, log) = store_with_log("dedup", &[("", "deploy", "部署用 docker")]);
    store.recall_relevant(Some("ws-session"), 5, &learn(true));
    let first = inject_row(&log, "ws-session").breakdown.clone();
    assert!(first.is_some(), "开档第一次就该有回执");
    // 第二次换一批词：若真发生写入，回执里的分数会跟着相关性信号变
    let mut q2 = learn(true);
    q2.terms = vec!["镜像".to_string()];
    store.recall_relevant(Some("ws-session"), 5, &q2);
    let injects: Vec<RecallRow> = rows(&log)
        .into_iter()
        .filter(|r| r.mode == "inject")
        .collect();
    assert_eq!(injects.len(), 1, "同键位第二次不许多出一行");
    assert_eq!(injects[0].breakdown, first, "第一次的回执被第二次覆盖了");
}

/// 存量库（Q10 之前的 9 列表）：开连接要**补列**、旧行读成 NULL、之后能写进新列。
/// `CREATE TABLE IF NOT EXISTS` 对已存在的表不补列，这条就是那条迁移的验收。
#[test]
fn a_legacy_table_gets_the_column_and_keeps_its_rows() {
    let dir = temp_dir("legacy");
    let path = dir.join("usage.sqlite3");
    {
        let conn = Connection::open(&path).unwrap();
        // 建表语句照 Q10 之前的 DDL 逐字抄（少一个 breakdown 列），不发明形状
        conn.execute_batch(
            "CREATE TABLE memory_inject_log (
                 id           INTEGER PRIMARY KEY AUTOINCREMENT,
                 ts           INTEGER NOT NULL,
                 mode         TEXT NOT NULL,
                 session_key  TEXT NOT NULL,
                 scope_id     TEXT NOT NULL,
                 n_candidates INTEGER NOT NULL,
                 returned     INTEGER NOT NULL,
                 keys         TEXT NOT NULL,
                 elapsed_us   INTEGER NOT NULL);
             CREATE INDEX idx_mil_mode_ts ON memory_inject_log(mode, ts);
             CREATE UNIQUE INDEX ux_mil_inject ON memory_inject_log(session_key, scope_id) WHERE mode = 'inject';
             INSERT INTO memory_inject_log (ts, mode, session_key, scope_id, n_candidates, returned, keys, elapsed_us)
             VALUES (1, 'inject', 'old-session', '', 3, 2, '[\"a\",\"b\"]', 42);",
        )
        .unwrap();
    }
    let log = RecallLog::at(&path);
    let old = rows(&log)
        .into_iter()
        .find(|r| r.session_key == "old-session")
        .expect("旧行还在 = 迁移没把表重建");
    assert_eq!(old.keys, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(old.breakdown, None, "老行没有回执 = NULL，不是空串");
    log.note_inject(
        "new-session",
        "",
        1,
        1,
        &["z".to_string()],
        std::time::Duration::from_micros(3),
        Some("{}"),
    )
    .unwrap();
    let fresh = rows(&log)
        .into_iter()
        .find(|r| r.session_key == "new-session")
        .expect("补列后新写要落得进去");
    assert_eq!(fresh.breakdown.as_deref(), Some("{}"));
}
