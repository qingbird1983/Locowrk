//! `inject_view::view` 的读端判据：全部打**生产本体**，不手搓一个等价的折算函数。
//!
//! ⚠ 这一簇原先有五条，判的全是"最近一次加权排序的回执怎么读"（跳过没算过的行 /
//! 坏字节降级 / 缺档补 0 / 空交付是正解不是损坏）。它们的数据源 = `memory_learn/` 写的
//! `breakdown` 那一列，随学习档整族退役（S3 刀③b · 裁定 = specs/10 开工三问②）⇒
//! **五条一起离场，而不是留着断言一个恒假的常量**。"坏字节要降级不许报错"那条纪律
//! 仍然成立，但它现在没有对象：读端已经不再解析库里任何自由文本，剩下的两个比率由
//! `RecallLog::stats()` 现算（那边有它自己的判据 = `recall_log/tests/metrics.rs`）。
//!
//! 活下来的两条判的是这块剩下的两件事：① L0 那三个派生读数如实透出去（含"零注入 =
//! `None` 而不是 0.0"那条口径）；② **IPC 载荷的顶层形状**——前端
//! `types/memory.ts` 的 `InjectView` 与这里必须同一份，后端漏一个恒空的 `explain`
//! 出去在 Rust 侧看不见，只有这一档看得见。

use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::recall_log::RecallLog;

fn log(tag: &str) -> Arc<RecallLog> {
    let d = crate::testutil::unique_dir(&format!("injectview-{tag}"));
    std::fs::create_dir_all(&d).expect("create inject_view temp dir");
    RecallLog::at(d.join("usage.sqlite3"))
}

fn dur() -> std::time::Duration {
    std::time::Duration::from_micros(5)
}

/// 汇总如实：两行注入（一行交付两条、一行空交付）⇒ 次数 2、空块率 0.5、
/// 注入过的 key 集合 2；没有一次检索回执 ⇒ 产出率是 **`Some(0.0)`** 而不是 `None`
/// （`None` 只留给"一个 key 都没注入过"那一种，见 `metrics.rs` 的口径判据）。
#[test]
fn the_view_reports_the_l0_counts() {
    let log = log("counts");
    log.note_inject("sess-a", "", 3, 2, &["k1".into(), "k2".into()], dur())
        .unwrap();
    log.note_inject("sess-b", "", 2, 0, &[], dur()).unwrap();
    let v = view(&log).unwrap();
    assert_eq!(v.overview.injections, 2);
    assert_eq!(
        v.overview.injected_keys, 2,
        "按 key 去重的集合，不是注入次数"
    );
    assert_eq!(v.overview.yielded_keys, 0, "一次检索回执都没有");
    assert_eq!(v.overview.store_empty_ratio, Some(0.5));
    assert_eq!(
        v.overview.injection_yield,
        Some(0.0),
        "注入了两个 key 而模型一个都没查回来 = 0.0；报 None 会把这说成「没注入过」"
    );
}

/// 顶层形状 = 只有 `overview`。学习档那套解释字段（`explain` 与其下的 `rows` /
/// `share` / `weights`）已经从契约里摘掉，前端类型同刀瘦身；这条钉住"后端不再往外送"。
#[test]
fn the_payload_has_no_explain_section_anymore() {
    let log = log("shape");
    log.note_inject("s", "", 1, 1, &["k".into()], dur())
        .unwrap();
    let json = serde_json::to_value(view(&log).unwrap()).unwrap();
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("载荷是对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["overview"],
        "解释档若还在载荷里，面板就在演一份没有来源的东西"
    );
}

/// 空表：`view` 不建库也不报错（`stats` 在 open 时幂等建表）。
#[test]
fn an_empty_log_reads_as_zero_not_as_an_error() {
    let d = crate::testutil::unique_dir("injectview-empty");
    std::fs::create_dir_all(&d).unwrap();
    let path: PathBuf = d.join("usage.sqlite3");
    let v = view(&RecallLog::at(&path)).unwrap();
    assert_eq!(v.overview.injections, 0);
    assert_eq!(
        v.overview.store_empty_ratio, None,
        "零注入 = None 而不是 0.0"
    );
    assert_eq!(v.overview.injection_yield, None);
}
