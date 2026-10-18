//! **注入记账**那一族（自 `md_read.rs` 搬出，S1 刀③b-2 起单独一簇）：这一簇管的不是
//! "读的是什么源"，而是**一次注入要么完整要么不发**——降级时照常交付其余（"注入不拦
//! 聊天"那条裁定优先于记账洁癖），但记账宁缺毋伪。
//!
//! 为什么从 `md_read` 拆出来（三问自检第①问）：那一条管**读源**（列举 / 召回 / 项目索引
//! 只读 md，把库改名也照常出数），这一条管**交付与埋点**——两件事各自都还在动，混在一
//! 个文件里到刀③b-2 时已经 408 行越过 400 闸。抬基线、压注释都算作弊，所以按面拆。
//! 用例本体逐字搬移，**断言一字未改**（搬动与改语义混进一份 diff 就没法用行守恒判据了）。
//!
//! 三档故障各有一条盯着，撤掉任一档的计数都会红：条目级（`read_to_string` 失败）、
//! 层级（`layers()` 的第二个返回值）、以及判据本体（`inject_loggable` 那个纯函数）。

use super::*;
use crate::memory::recall::{inject_loggable, RecallOutcome};
use crate::memory::MemoryRow;
use crate::recall_log::RecallLog;

/// 埋点判据的两档（纯函数，不需要接 `RecallLog` 就有可观测面）。
/// 这一档要是漂成"总是记"，`memory_inject_log` 的使用率会系统性偏高而**没有任何
/// 一条行为测试会红**——所以它必须在这儿钉住。
///
/// 原本是三档：完整 / partial / `Err`（源不可用）。刀②c-1 起 `RecallOutcome` 不再是
/// `Result`，`Err` 那半**没有对应物可测**了：文件制里"整个源不可用"表现为"一层都扫不动"，
/// 而层级故障现在也进 `unreadable` 账（`memlayout::layers` 的第二返回值）⇒ 它就是
/// `partial = true`。一档盖住两种故障，不是少测了一档。
#[test]
fn inject_loggable_requires_a_complete_reading() {
    let row = || MemoryRow {
        key: "k".into(),
        value: "v".into(),
        tags: String::new(),
        source: "explicit".into(),
        workspace: String::new(),
        updated_at: 1,
    };
    let outcome = |partial: bool| RecallOutcome {
        rows: vec![row()],
        n_candidates: 3,
        partial,
    };
    assert!(inject_loggable(&outcome(false)), "完整读数才记");
    assert!(
        !inject_loggable(&outcome(true)),
        "有扫不动的部分（条目或整层）⇒ 不记：候选数偏低就是指标造假"
    );
}

/// 有一条扫不动时**注入照常交付其余**（"注入不拦聊天"那条裁定优先于记账洁癖）。
/// 本条只管交付；"那一轮不写埋点"由下面那条接 `RecallLog` 的用乐视，判据本身
/// 再由 `inject_loggable_requires_a_complete_reading` 单独钉。
#[test]
fn recall_still_delivers_the_readable_ones_when_a_fact_is_unreadable() {
    let dir = temp_dir("recall-partial");
    let store = wired(&dir);
    let ws = keysid();
    store
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();
    // 一个"事实文件"其实是目录 ⇒ 文件级 read 失败 ⇒ 扫描带 partial
    std::fs::create_dir_all(
        dir.join("memory")
            .join("projects")
            .join(plugin_api::memlayout::project_dir_name(&ws))
            .join("facts")
            .join("broken.md"),
    )
    .unwrap();

    let rows = store.recall_quiet(None, 10);
    assert_eq!(rows.len(), 1, "扫不动一条不许连累其余：{rows:?}");
    assert_eq!(rows[0].key, "pref.ok");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 有扫不动的条目时**不写埋点行**。观测量落成文件行数（进程内计数换个进程就没了），
/// 所以这里读的是 `memory_inject_log` 的真实计数。
///
/// 上一条纯函数证的是判据本身，这一条证的是**判据真的接到了召回通路上**：少了它，
/// `partial: scan.unreadable > 0` 漂成恒 false 也不会有任何测试报红（这条红过一次就是
/// 这么发现的——第一版两轮都用 `None` 会话跑，而 `ux_mil_inject` 的唯一键是
/// `(session_key, scope_id)`：第二行被 `INSERT OR IGNORE` 吃掉，于是"仍然只有一行"
/// 在两种实现下都成立 = 零证明力）。所以这里两轮各用一个真实会话 id，并配一组
/// **正对照**：同样的两轮、没有扫不动条目时必须记到第二行，否则上面那个"= 1"
/// 也可能只是因为埋点通路整条没接上。
#[test]
fn an_incomplete_scan_writes_no_inject_row() {
    let dir = temp_dir("recall-partial-log");
    let log = RecallLog::at(dir.join("usage.sqlite3"));
    let store = wired(&dir).with_recall_log(log.clone());
    store
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();

    store.recall_quiet(Some("ws-session"), 10);
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "前提一：完整读数确实记了一行"
    );

    // 全局层里塞一个"文件"其实是目录 ⇒ 扫描带 partial
    std::fs::create_dir_all(dir.join("memory").join("global").join("broken.md")).unwrap();
    let rows = store.recall_quiet(Some("ws-variant-session"), 10);
    assert_eq!(rows.len(), 1, "注入不拦聊天：照样交付读得到的那些");
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "partial 那一轮不许记第二行（宁缺毋伪接到通路上）"
    );

    // 正对照：同两轮、无扫不动条目 ⇒ 必须记两行
    let dir2 = temp_dir("recall-full-log");
    let log2 = RecallLog::at(dir2.join("usage.sqlite3"));
    let store2 = wired(&dir2).with_recall_log(log2.clone());
    store2
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();
    store2.recall_quiet(Some("ws-session"), 10);
    store2.recall_quiet(Some("ws-variant-session"), 10);
    assert_eq!(
        log2.stats().unwrap().injections,
        2,
        "正对照：完整两轮就该记两行（否则上面那个 1 是空集里的真话）"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

/// 整层扫不动（`projects/` 那个位置被一个普通文件冒充）：全局层照常交付，而那一轮
/// **不写埋点行**。
///
/// 它与 `an_incomplete_scan_writes_no_inject_row` 分属**两档故障**：那一条由
/// `read_to_string` 失败进账（文件级），这一条由 `layers()` 的第二个返回值进账
/// （层级）。只留一条的话，撤掉任一档的计数都不会红——而层级那一档在真源 = 文件之后
/// 是最要命的（整层读不动时用户看到的是"我好像从没记过东西"）。
#[test]
fn a_blocked_layer_also_suppresses_the_inject_row() {
    let dir = temp_dir("recall-blocked-layer");
    let log = RecallLog::at(dir.join("usage.sqlite3"));
    let store = wired(&dir).with_recall_log(log.clone());
    store
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();

    store.recall_quiet(Some("ws-session"), 10);
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "前提：完整那一轮记了一行"
    );

    std::fs::write(dir.join("memory").join("projects"), "not a directory").unwrap();
    let rows = store.recall_quiet(Some("ws-variant-session"), 10);
    assert_eq!(rows.len(), 1, "注入不拦聊天：全局层那条照常交付：{rows:?}");
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "整层扫不动那一轮同样不许记第二行"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
