//! usage.sqlite3 单测（⑩③/⑫）：upsert 累计、first_used_at 保留首用、
//! attach 投影 join、EWMA 衰减积累、hot_tools / ewma_scores 查询
//! （exposure 文档 §3.1/§3.2/§5.1/§10.1）。

use std::collections::HashMap;

use super::{UsageDb, UsageRecorder, attach, update_ewma};
use crate::usage::UsageEwma;
use kernel_agent::GlobalEwma;
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

/// §5.1 公式：同窗口 decay=1 纯加法；隔 7 天衰减 e⁻¹。
#[test]
fn ewma_decays_by_seven_day_half_life_formula() {
    let day = 86_400;
    assert!((update_ewma(0.0, 1_000, 1_000, 1) - 1.0).abs() < 1e-9);
    // 7 天前 1.0 → 衰减 e⁻¹ ≈ 0.3679，再加本次 1.0
    let decayed = update_ewma(1.0, 1_000 + 7 * day, 1_000, 1);
    assert!((decayed - (1.0 * (-1.0f64).exp() + 1.0)).abs() < 1e-9);
    // 乱序时间戳不产生负天数
    assert!((update_ewma(1.0, 1_000, 2_000, 1) - 2.0).abs() < 1e-9);
}

/// 跨窗口积累：先记一次，7 天后再记 → ewma = e⁻¹ + 1。
#[test]
fn flush_accumulates_ewma_with_decay() {
    let day = 86_400;
    let db = temp_db("ewma");
    db.record("fs_read", "tool-fs", true, 1_000);
    db.record("fs_read", "tool-fs", true, 1_000 + 7 * day);
    let conn = db.conn.lock().unwrap();
    let (score, last): (f64, i64) = conn
        .query_row(
            "SELECT ewma_score, last_used_at FROM tool_usage WHERE tool_name = 'fs_read'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let expected = (-1.0f64).exp() + 1.0;
    assert!((score - expected).abs() < 1e-9, "score={score}");
    assert_eq!(last, 1_000 + 7 * day);
}

/// hot_tools 按调用次数降序；ewma_scores 全表映射；GlobalEwma 适配器透传。
#[test]
fn hot_tools_and_ewma_scores_query() {
    let db = temp_db("query");
    for i in 0..3 {
        db.record("hot", "p", true, 1_000 + i);
    }
    db.record("cold", "p", true, 2_000);
    assert_eq!(db.hot_tools(1).unwrap(), vec![("hot".into(), 3)]);
    let scores = db.ewma_scores().unwrap();
    assert_eq!(scores.len(), 2);
    assert!((scores["hot"] - 3.0).abs() < 1e-3, "三次落库衰减累计");

    let adapter = UsageEwma::new(UsageRecorder::direct(db));
    let via_trait = GlobalEwma::ewma_scores(adapter.as_ref());
    assert!((via_trait["cold"] - 1.0).abs() < 1e-3);
}
