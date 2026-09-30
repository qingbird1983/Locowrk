//! merge_catalog 单测（⑩①②）：hidden 剔除 / 同名先到先得 / always 计数。

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
    let (merged, always) = merge_catalog(Vec::new(), incoming.into_iter());
    let names: Vec<&str> = merged.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "d"], "hidden 剔除，其余保持顺序");
    assert_eq!(always, 2);
}

#[test]
fn same_name_first_declaration_wins() {
    let existing = vec![spec("a", Exposure::OnDemand)];
    let incoming = [spec("a", Exposure::Always), spec("z", Exposure::OnDemand)];
    let (merged, always) = merge_catalog(existing, incoming.into_iter());
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].exposure, Exposure::OnDemand, "先声明者赢");
    assert_eq!(merged[1].name, "z");
    assert_eq!(always, 0, "被顶掉的 always 不计数");
}
