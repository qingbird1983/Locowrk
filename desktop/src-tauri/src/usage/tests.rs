//! usage.sqlite3 单测（⑩③）：upsert 累计、first_used_at 保留首用、
//! attach 投影 join（exposure 文档 §3.1/§3.2/§10.1）。

use std::collections::HashMap;

use super::{attach, UsageDb};
use plugin_service::{InstallSource, InstalledPlugin};

fn temp_db(tag: &str) -> UsageDb {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "locowrk-usage-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    UsageDb::open(&dir.join("usage.sqlite3")).expect("open temp usage db")
}

fn installed(id: &str, source: InstallSource) -> InstalledPlugin {
    InstalledPlugin {
        id: id.into(),
        version: "1.2.0".into(),
        enabled: true,
        installed_at: 100,
        source,
        active_version: Some("1.2.0".into()),
        loaded_versions: vec!["1.2.0".into()],
        display: Default::default(),
    }
}

#[test]
fn tool_upsert_accumulates_counts_and_last_ts() {
    let db = temp_db("tool");
    db.record("fs_read", "tool-fs", true, 1_000);
    db.record("fs_read", "tool-fs", true, 2_000);
    db.record("fs_read", "tool-fs", false, 3_000);
    let conn = db.conn.lock().unwrap();
    let (calls, ok, last, plugin): (i64, i64, i64, String) = conn
        .query_row(
            "SELECT call_count, success_count, last_used_at, plugin_id
             FROM tool_usage WHERE tool_name = 'fs_read'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!((calls, ok, last), (3, 2, 3_000));
    assert_eq!(plugin, "tool-fs");
}

#[test]
fn plugin_upsert_keeps_first_used_updates_last() {
    let db = temp_db("plugin");
    db.record("fs_read", "tool-fs", true, 1_000);
    db.record("fs_write", "tool-fs", true, 9_000);
    let times = db.plugin_times().unwrap();
    assert_eq!(
        times.get("tool-fs"),
        Some(&(1_000, 9_000)),
        "first 保留首用"
    );
}

#[test]
fn attach_joins_usage_and_marks_builtin_unassessed() {
    let db = temp_db("attach");
    db.record("fs_read", "tool-fs", true, 1_000);
    let times = db.plugin_times().unwrap();

    let installed = vec![
        installed("tool-fs", InstallSource::Market),
        installed("tool-echo", InstallSource::Builtin),
    ];
    let rows = attach(&installed, 500_000, &times);
    assert_eq!(rows.len(), 2);

    let fs = rows.iter().find(|r| r.id == "tool-fs").unwrap();
    assert_eq!(fs.first_used_at, Some(1_000));
    assert_eq!(fs.last_used_at, Some(1_000));
    assert!(fs.idle.is_some(), "安装态插件评估闲置");
    assert!(fs.active);
    assert!(fs.filter_key.is_some());

    let echo = rows.iter().find(|r| r.id == "tool-echo").unwrap();
    assert!(echo.idle.is_none(), "builtin 开发态不评估闲置");
    assert!(echo.filter_key.is_none());
}

/// 空统计表 = 全 None（插件装了但一次没用过，§8.7 路径）。
#[test]
fn attach_with_empty_times_gives_never_used_path() {
    let times: HashMap<String, (i64, i64)> = HashMap::new();
    let rows = attach(
        &[installed("tool-fs", InstallSource::Market)],
        500_000,
        &times,
    );
    let fs = &rows[0];
    assert_eq!(fs.first_used_at, None);
    assert_eq!(fs.last_used_at, None);
    assert!(fs.idle.is_some());
}
