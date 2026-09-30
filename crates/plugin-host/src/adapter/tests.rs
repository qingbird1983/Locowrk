//! merge_catalog 单测（⑩①②）：hidden 剔除 / 同名先到先得 / always 计数 /
//! 非法名剔除（OpenAI 兼容后端硬约束）。

use plugin_api::{Exposure, SideEffect, ToolSpec};
use serde_json::json;

use super::merge_catalog;

fn spec(name: &str, exposure: Exposure) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: String::new(),
        schema: json!({}),
        side_effect: SideEffect::Pure,
        exposure,
    }
}

#[test]
fn hidden_is_dropped_and_always_counted() {
    let incoming = [
        spec("a", Exposure::Always),
        spec("b", Exposure::OnDemand),
        spec("c", Exposure::Hidden),
        spec("d", Exposure::Always),
    ];
    let (merged, always, invalid) = merge_catalog(Vec::new(), incoming.into_iter(), false);
    let names: Vec<&str> = merged.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "d"], "hidden 剔除，其余保持顺序");
    assert_eq!(always, 2);
    assert!(invalid.is_empty());
}

#[test]
fn same_name_first_declaration_wins() {
    let existing = vec![spec("a", Exposure::OnDemand)];
    let incoming = [spec("a", Exposure::Always), spec("z", Exposure::OnDemand)];
    let (merged, always, invalid) = merge_catalog(existing, incoming.into_iter(), false);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].exposure, Exposure::OnDemand, "先声明者赢");
    assert_eq!(merged[1].name, "z");
    assert_eq!(always, 0, "被顶掉的 always 不计数");
    assert!(invalid.is_empty());
}

#[test]
fn invalid_names_are_skipped_and_reported() {
    let incoming = [
        spec("ok_1", Exposure::OnDemand),
        spec("bad.name", Exposure::OnDemand),
        spec("bad name", Exposure::Always),
    ];
    let (merged, always, invalid) = merge_catalog(Vec::new(), incoming.into_iter(), false);
    let names: Vec<&str> = merged.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["ok_1"], "非法名不进目录（会让整个请求 400）");
    assert_eq!(always, 0, "被剔除的 always 不计数");
    assert_eq!(invalid, ["bad.name", "bad name"], "回报给调用方告警");
}

#[test]
fn include_hidden_keeps_hidden_but_not_always_count() {
    // ⑫ search_tools 的全目录索引：hidden 可检索；always 计数只归常规目录
    let incoming = [spec("a", Exposure::Always), spec("c", Exposure::Hidden)];
    let (merged, always, invalid) = merge_catalog(Vec::new(), incoming.into_iter(), true);
    let names: Vec<&str> = merged.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["a", "c"], "include_hidden 保留 hidden");
    assert_eq!(always, 1, "always 预算仍只数常规目录");
    assert!(invalid.is_empty());
}
