//! S3 刀③b 落盘面：本机那些**已经带着 `breakdown` 列**的观测表，在代码不再读写这一列
//! 之后必须照常开库、读旧行、写新行。
//!
//! 为什么这条留在本簇而不是随 `breakdown.rs` 那一族一起删掉：`CREATE TABLE IF NOT EXISTS`
//! 对已存在的表**不删列**，所以存量库（`<数据根>/usage.sqlite3`）里那一列还在场。摘掉写侧
//! 之后它变成"列在场、没人读写"——这本身不是故障，**前提是没有任何语句假设列的个数**：
//! 写侧 `INSERT` 点名八列、读侧 `SELECT` 点名七列，两处都对多出来的列免疫。这条用例就是
//! 那个前提的机器证明；删掉它等于把"旧库还能用"降级成一句口头结论。
//!
//! 与 `check-schema-parity.mjs` 的 H1/H2 那两档分工：那边管 `memories` 表的学习两列
//! （同样"在场但没人读写"，去处随 S4 一起裁），这边管观测表这一列的**读写通路不受影响**。

use std::time::Duration;

use crate::recall_log::RecallLog;
use rusqlite::Connection;

use super::{rows, temp_dir};

#[test]
fn a_legacy_table_with_the_retired_column_still_opens_reads_and_writes() {
    let dir = temp_dir("legacy-cols");
    let path = dir.join("usage.sqlite3");
    {
        // 建表串照退役前那一版 DDL 逐字抄（= 现 DDL + 一枚可空 `breakdown`），
        // 旧行连回执一起写进去——夹具必须像真实现场，不然这条判据证的是空气。
        let conn = Connection::open(&path).expect("open legacy usage db");
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
                 elapsed_us   INTEGER NOT NULL,
                 breakdown    TEXT);
             CREATE INDEX idx_mil_mode_ts ON memory_inject_log(mode, ts);
             CREATE UNIQUE INDEX ux_mil_inject ON memory_inject_log(session_key, scope_id) WHERE mode = 'inject';
             INSERT INTO memory_inject_log (ts, mode, session_key, scope_id, n_candidates, returned, keys, elapsed_us, breakdown)
             VALUES (1, 'inject', 'old-session', '', 3, 2, '[\"a\",\"b\"]', 42, '{\"rows\":[]}');",
        )
        .expect("seed legacy row");
        drop(conn);
    }
    // 现 DDL 在这张表上重跑：`IF NOT EXISTS` 不重建、不删列 ⇒ 旧行原样在场。
    let log = RecallLog::at(&path);
    let old = rows(&log)
        .into_iter()
        .find(|r| r.session_key == "old-session")
        .expect("旧行还在 = 摘掉那一列没有把表重建");
    assert_eq!(old.keys, vec!["a".to_string(), "b".to_string()]);
    assert_eq!((old.n_candidates, old.returned), (3, 2));

    // 新形状写一行：只点名当前那八列 ⇒ 多出来的 `breakdown` 落它自己的 NULL 默认，
    // 既不报错也不串位（串位的话下面那条 `returned` 断言就红了）。
    log.note_inject(
        "new-session",
        "",
        1,
        1,
        &["z".to_string()],
        Duration::from_micros(3),
    )
    .expect("write under the new column list");
    let fresh = rows(&log)
        .into_iter()
        .find(|r| r.session_key == "new-session")
        .expect("新行落得进去");
    assert_eq!(
        (fresh.mode.as_str(), fresh.n_candidates, fresh.returned),
        ("inject", 1, 1),
        "列数变了而值串了位 = 读侧在按位置猜"
    );
    assert_eq!(
        rows(&log).len(),
        2,
        "两个键位各一行：旧行没被新写挤掉，去重索引也没因列变化而失效"
    );
}
