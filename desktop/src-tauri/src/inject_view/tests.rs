//! `inject_view::view` 的读端判据：全部打**生产本体**，不手搓一个等价的解析函数。
//!
//! 三条口径各一条：①跳过没有回执的行（关档注入照常埋点但 `breakdown` 是 NULL）；
//! ②坏字节降级为 `unreadable` 而不是把整条通路报错；③读端不假设库里那一串与自己
//! 同版本——缺字段按 0 读，不 panic 也不整行丢弃。

use std::path::PathBuf;

use super::*;
use crate::recall_log::RecallLog;

fn log(tag: &str) -> std::sync::Arc<RecallLog> {
    let d = crate::testutil::unique_dir(&format!("injectview-{tag}"));
    std::fs::create_dir_all(&d).expect("create inject_view temp dir");
    RecallLog::at(d.join("usage.sqlite3"))
}

/// ③开档那一次的回执：`{"weights":[…],"rows":[{"key","share","total"},…]}` 的最小形。
fn receipt(keys: &[&str]) -> String {
    serde_json::json!({
        "weights": [400, 250, 200, 150],
        "rows": keys
            .iter()
            .map(|k| serde_json::json!({"key": k, "share": [700, 100, 100, 100], "total": 123}))
            .collect::<Vec<_>>(),
    })
    .to_string()
}

fn dur() -> std::time::Duration {
    std::time::Duration::from_micros(5)
}

/// ①**最近一次"真算过分"的注入**，不是"最近一次注入"：会话 B 关档在后（NULL），
/// 会话 A 开档在前，面板必须报 A——报"没什么可说"就是把用户想看的那条挡掉了。
#[test]
fn view_skips_injections_that_were_never_scored() {
    let log = log("skip");
    log.note_inject(
        "sess-a",
        "",
        3,
        2,
        &["k1".into(), "k2".into()],
        dur(),
        Some(&receipt(&["k1", "k2"])),
    )
    .unwrap();
    log.note_inject("sess-b", "", 3, 1, &["k1".into()], dur(), None)
        .unwrap();
    let v = view(&log).unwrap();
    assert!(v.explain.present);
    assert_eq!(
        v.explain.session_key, "sess-a",
        "面板报的不是那一次真算过分的注入"
    );
    assert_eq!(v.explain.weights, [400, 250, 200, 150]);
    assert_eq!(v.explain.rows.len(), 2, "回执里几条交付就显示几条");
    assert_eq!(v.explain.rows[0].share, [700, 100, 100, 100]);
    assert!(v.explain.ts > 0, "面板要能说这是哪一次的注入");
    // 汇总走的是整张表，不是被选中的那一行
    assert_eq!(v.overview.injections, 2);
}

/// ②一次都没算过 ⇒ `present = false`（面板走空态）。同时**汇总仍要如实**：
/// "没有解释"与"没有注入"是两件事，靠 `overview.injections` 分开。
#[test]
fn view_says_absent_without_inventing_an_explanation() {
    let log = log("absent");
    log.note_inject("sess-off", "", 1, 1, &["k".into()], dur(), None)
        .unwrap();
    let v = view(&log).unwrap();
    assert!(!v.explain.present);
    assert!(!v.explain.unreadable, "没有回执不是坏回执");
    assert!(v.explain.rows.is_empty());
    assert_eq!(v.overview.injections, 1, "空态不许顺手把行数报成 0");
    assert_eq!(v.overview.store_empty_ratio, Some(0.0));
}

/// ③坏字节：**降级而不是报错**。库里那一串读不成 JSON（手改表 / 更早版本的字节）
/// 时面板要说"这一条读不出来"，而汇总照给——整条通路报错 = 观测面判死自己的数据源。
#[test]
fn a_corrupt_receipt_degrades_rather_than_failing_the_whole_view() {
    let log = log("corrupt");
    log.note_inject(
        "sess-x",
        "",
        2,
        2,
        &["k1".into(), "k2".into()],
        dur(),
        Some("{ not json ],"),
    )
    .unwrap();
    let v = view(&log).unwrap();
    assert!(v.explain.present, "有回执这件事本身要如实");
    assert!(v.explain.unreadable);
    assert_eq!(v.overview.injections, 1, "坏回执不许把汇总带崩");
}

/// 读端不假设库里那一串与自己同版本：缺 `share` / 缺 `total` 按 0 读，多余的档位
/// 截掉；键名不是字符串的那条整条跳过而不是把一半的行丢掉。
#[test]
fn a_partial_receipt_reads_as_far_as_it_can() {
    let log = log("partial");
    log.note_inject(
        "sess-p",
        "",
        3,
        3,
        &["a".into(), "b".into(), "c".into()],
        dur(),
        Some(r#"{"weights":[1,2],"rows":[{"key":"a","share":[5,6]},{"total":9},{"key":"c","share":[1,2,3,4,5],"total":7}]}"#),
    )
    .unwrap();
    let v = view(&log).unwrap();
    assert!(!v.explain.unreadable);
    assert_eq!(v.explain.weights, [1, 2, 0, 0], "缺的档位补 0，不报错");
    let keys: Vec<&str> = v.explain.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["a", "c"], "没有键名的一行无法解释，跳过它");
    assert_eq!(v.explain.rows[0].share, [5, 6, 0, 0]);
    assert_eq!(v.explain.rows[1].share, [1, 2, 3, 4], "多余的档位按 4 档截");
    assert_eq!(v.explain.rows[1].total, 7);
}

/// **空交付不是坏数据**：`{"rows":[]}` 是"那一次什么都没交付"的正解（Q1 的空块率
/// 判的就是这种行）。把它读成 `unreadable` = 把空注入演成数据损坏，面板会说错话。
#[test]
fn an_empty_delivery_is_a_valid_receipt_not_a_corrupt_one() {
    let log = log("empty-delivery");
    log.note_inject(
        "sess-e",
        "",
        0,
        0,
        &[],
        dur(),
        Some(r#"{"weights":[400,250,200,150],"rows":[]}"#),
    )
    .unwrap();
    let v = view(&log).unwrap();
    assert!(v.explain.present);
    assert!(
        !v.explain.unreadable,
        "空交付被读成坏字节 = 空块率那一档演成数据损坏"
    );
    assert!(v.explain.rows.is_empty());
    assert_eq!(v.overview.store_empty_ratio, Some(1.0));
}

/// 空表：`view` 不建库也不报错（`recent`/`stats` 各自 open 时幂等建表）。
#[test]
fn an_empty_log_reads_as_zero_not_as_an_error() {
    let d = crate::testutil::unique_dir("injectview-empty");
    std::fs::create_dir_all(&d).unwrap();
    let path: PathBuf = d.join("usage.sqlite3");
    let v = view(&RecallLog::at(&path)).unwrap();
    assert!(!v.explain.present);
    assert_eq!(v.overview.injections, 0);
    assert_eq!(
        v.overview.store_empty_ratio, None,
        "零注入 = None 而不是 0.0"
    );
    assert_eq!(v.overview.injection_yield, None);
}
