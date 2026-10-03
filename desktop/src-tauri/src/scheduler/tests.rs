//! ⑭ 调度器单测：store CRUD/到期/上限、工具面参数校验与目标解析纯函数。
//! 引擎循环本身是薄 IO（try_lock + run_turn），决策已抽成纯函数覆盖。

use std::path::PathBuf;
use std::sync::Arc;

use plugin_api::{CallCtx, Tool, ToolError};
use serde_json::{json, Value};

use super::engine::{defer_or_give_up, next_interval_run, resolve_target, Target};
use super::store::{Kind, Schedule, ScheduleDb};
use super::tools::{ScheduleCreateTool, ScheduleDeleteTool, ScheduleListTool};

fn sched(id: &str, kind: Kind, run_at: i64) -> Schedule {
    Schedule {
        id: id.into(),
        session_id: "desktop-1".into(),
        kind,
        title: "t".into(),
        prompt: "p".into(),
        run_at,
        interval_secs: if kind == Kind::Interval {
            Some(60)
        } else {
            None
        },
        deferrals: 0,
        created_ms: 0,
    }
}

fn tmp_db(tag: &str) -> (PathBuf, Arc<ScheduleDb>) {
    // 无 tempfile 依赖（memory/experiences 测试同款）：pid 隔离 + 收尾删
    let dir = std::env::temp_dir().join(format!("locowrk-sched-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let db = Arc::new(ScheduleDb::open(&dir.join("schedule.sqlite3")).expect("open schedule db"));
    (dir, db)
}

#[test]
fn create_list_due_delete_roundtrip() {
    let (_dir, db) = tmp_db("roundtrip");
    db.create(&sched("a", Kind::Once, 100)).unwrap();
    db.create(&sched("b", Kind::Interval, 200)).unwrap();

    let mine = db.list_session("desktop-1");
    assert_eq!(mine.len(), 2);
    assert!(db.list_session("other").is_empty());

    assert_eq!(db.due(199).len(), 1, "only the run_at=100 one is due");
    assert_eq!(db.due(200).len(), 2);

    assert!(db.delete("a"));
    assert!(!db.delete("a"), "second delete is a no-op");
    assert_eq!(db.list_session("desktop-1").len(), 1);
}

#[test]
fn create_enforces_cap() {
    let (_dir, db) = tmp_db("cap");
    for i in 0..super::store::MAX_SCHEDULES {
        db.create(&sched(&format!("s{i}"), Kind::Once, 1)).unwrap();
    }
    let err = db.create(&sched("overflow", Kind::Once, 1)).unwrap_err();
    assert!(
        err.contains("full"),
        "cap error should mention fullness: {err}"
    );
}

#[test]
fn reschedule_updates_row() {
    let (_dir, db) = tmp_db("resched");
    db.create(&sched("a", Kind::Interval, 100)).unwrap();
    db.reschedule("a", 500, 3);
    let row = &db.list_session("desktop-1")[0];
    assert_eq!(row.run_at, 500);
    assert_eq!(row.deferrals, 3);
}

// ---- 工具面 ----

fn ctx(session: Option<&str>) -> CallCtx {
    let mut c = CallCtx::new();
    c.session_id = session.map(str::to_owned);
    c
}

async fn call(t: &dyn Tool, c: &CallCtx, args: Value) -> Result<Value, ToolError> {
    t.call(c, args).await
}

#[tokio::test]
async fn create_requires_session_and_one_mode() {
    let (_dir, db) = tmp_db("create");
    let tool = ScheduleCreateTool::new(Arc::clone(&db));

    let err = call(
        tool.as_ref(),
        &ctx(None),
        json!({ "prompt": "x", "delay_secs": 5 }),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("no session context"));

    let err = call(
        tool.as_ref(),
        &ctx(Some("desktop-1")),
        json!({ "prompt": "x" }),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("interval_secs"), "{err}");

    let err = call(
        tool.as_ref(),
        &ctx(Some("desktop-1")),
        json!({ "prompt": "x", "at": 1 }), // 远古时刻
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("past"), "{err}");

    let ok = call(
        tool.as_ref(),
        &ctx(Some("desktop-1")),
        json!({ "prompt": "check the build", "delay_secs": 60 }),
    )
    .await
    .unwrap();
    assert!(ok["id"].as_str().unwrap().starts_with("sch-"));
    assert_eq!(ok["kind"], "once");
    assert_eq!(db.list_session("desktop-1").len(), 1);
}

#[tokio::test]
async fn create_interval_min_and_defaults() {
    let (_dir, db) = tmp_db("interval");
    let tool = ScheduleCreateTool::new(Arc::clone(&db));

    let err = call(
        tool.as_ref(),
        &ctx(Some("s")),
        json!({ "prompt": "x", "interval_secs": 3 }),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("10..=31536000"), "{err}");

    let ok = call(
        tool.as_ref(),
        &ctx(Some("s")),
        json!({ "prompt": "tick", "interval_secs": 10 }),
    )
    .await
    .unwrap();
    assert_eq!(ok["kind"], "interval");
    let row = &db.list_session("s")[0];
    assert_eq!(row.interval_secs, Some(10));
    assert!(
        row.run_at > super::store::now_ms(),
        "default start = now + interval"
    );
}

#[tokio::test]
async fn list_and_delete_are_session_scoped() {
    let (_dir, db) = tmp_db("scoped");
    let list = ScheduleListTool::new(Arc::clone(&db));
    let del = ScheduleDeleteTool::new(Arc::clone(&db));
    db.create(&sched("mine", Kind::Once, 100)).unwrap();
    db.create(&sched("theirs", Kind::Once, 100)).unwrap();
    // 把 theirs 划到别的会话：直改库（store 无 update-session 口径，重插）
    db.delete("theirs");
    let mut other = sched("theirs", Kind::Once, 100);
    other.session_id = "desktop-2".into();
    db.create(&other).unwrap();

    let out = call(list.as_ref(), &ctx(Some("desktop-1")), json!({}))
        .await
        .unwrap();
    let ids: Vec<&str> = out["schedules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["mine"]);

    let err = call(
        del.as_ref(),
        &ctx(Some("desktop-1")),
        json!({ "id": "theirs" }),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");

    let ok = call(
        del.as_ref(),
        &ctx(Some("desktop-1")),
        json!({ "id": "mine" }),
    )
    .await
    .unwrap();
    assert_eq!(ok["deleted"], "mine");
    assert!(db.list_session("desktop-1").is_empty());
}

// ---- 引擎纯决策 ----

#[test]
fn resolve_target_prefers_live_session_then_last_chat() {
    assert!(matches!(resolve_target(true, None), Target::Stored));
    assert!(matches!(resolve_target(true, Some("x")), Target::Stored));
    match resolve_target(false, Some("live-9")) {
        Target::Active(id) => assert_eq!(id, "live-9"),
        _ => panic!("expected Active"),
    }
    assert!(matches!(resolve_target(false, None), Target::Wait));
}

#[test]
fn defer_caps_by_kind_then_gives_up_or_skips() {
    use super::engine::OnDefer;
    // 一次性：240 次内顺延，之后放弃
    assert!(matches!(
        defer_or_give_up(Kind::Once, 0, 1000, 0),
        OnDefer::Retry(31000)
    ));
    assert!(matches!(
        defer_or_give_up(Kind::Once, 239, 1000, 0),
        OnDefer::Retry(31000)
    ));
    assert!(matches!(
        defer_or_give_up(Kind::Once, 240, 1000, 0),
        OnDefer::GiveUp
    ));
    // 周期：20 次内顺延，之后跳槽（deferrals 由调用方归零）
    assert!(matches!(
        defer_or_give_up(Kind::Interval, 19, 1000, 60_000),
        OnDefer::Retry(31000)
    ));
    match defer_or_give_up(Kind::Interval, 20, 1000, 60_000) {
        OnDefer::SkipSlot(at) => assert_eq!(at, 61_000),
        _ => panic!("expected SkipSlot"),
    }
}

#[test]
fn interval_next_run_never_earlier_than_old_plus_interval() {
    assert_eq!(next_interval_run(10_000, 5_000, 60_000), 70_000);
    // 补跑（now 远超旧槽 + 间隔）：从现在起算，不追帧连发
    assert_eq!(next_interval_run(1_000_000, 5_000, 60_000), 1_060_000);
}
