//! `tool` 模块单测：serde 形状契约。

use serde_json::json;

use super::{CallId, Exposure, SideEffect, ToolSpec};

#[test]
fn side_effect_json_is_tagged_snake_case() {
    let pure = serde_json::to_value(SideEffect::Pure).unwrap();
    assert_eq!(pure, json!({ "kind": "pure" }));

    let comp = serde_json::to_value(SideEffect::Compensable {
        undo_tool: "undo_http".into(),
    })
    .unwrap();
    assert_eq!(
        comp,
        json!({ "kind": "compensable", "undo_tool": "undo_http" })
    );

    let back: SideEffect = serde_json::from_value(comp).unwrap();
    assert_eq!(
        back,
        SideEffect::Compensable {
            undo_tool: "undo_http".into()
        }
    );
}

#[test]
fn call_id_is_transparent_string() {
    let id = CallId("call-1".into());
    let v = serde_json::to_value(&id).unwrap();
    assert_eq!(v, json!("call-1"));
    let back: CallId = serde_json::from_value(v).unwrap();
    assert_eq!(back, id);
}

#[test]
fn abi_version_is_frozen_at_1() {
    assert_eq!(crate::PLUGIN_ABI_VERSION, 1);
}

#[test]
fn exposure_defaults_to_on_demand_and_parses_all_values() {
    // 缺省字段 = on_demand（exposure 文档 §4.1：作者不写就当候选池）
    let without: ToolSpec = serde_json::from_value(json!({
        "name": "t",
        "schema": {},
        "side_effect": "Pure"
    }))
    .unwrap();
    assert_eq!(without.exposure, Exposure::OnDemand);

    for (raw, expected) in [
        ("always", Exposure::Always),
        ("on_demand", Exposure::OnDemand),
        ("hidden", Exposure::Hidden),
    ] {
        let spec: ToolSpec = serde_json::from_value(json!({
            "name": "t",
            "schema": {},
            "side_effect": "Pure",
            "exposure": raw
        }))
        .unwrap();
        assert_eq!(spec.exposure, expected, "exposure = {raw}");
    }

    // 未知值是作者错误：宁可报错也不静默归池（与 side_effect 未知值同策略）
    assert!(
        serde_json::from_value::<ToolSpec>(json!({
            "name": "t",
            "schema": {},
            "side_effect": "Pure",
            "exposure": "sometimes"
        }))
        .is_err()
    );
}

#[test]
fn exposure_serializes_snake_case() {
    let spec = ToolSpec {
        name: "t".into(),
        description: String::new(),
        schema: json!({}),
        side_effect: SideEffect::Pure,
        exposure: Exposure::Always,
    };
    assert_eq!(
        serde_json::to_value(&spec).unwrap()["exposure"],
        json!("always")
    );
}

#[test]
fn valid_tool_name_matches_openai_pattern() {
    // `^[a-zA-Z0-9_-]+$`：OpenAI 兼容后端的硬约束
    assert!(super::is_valid_tool_name("fs_read"));
    assert!(super::is_valid_tool_name("mcp_ctx7_resolve-library-id"));
    assert!(super::is_valid_tool_name("T9"));
    assert!(!super::is_valid_tool_name("git.undo_commit"));
    assert!(!super::is_valid_tool_name("has space"));
    assert!(!super::is_valid_tool_name("中文"));
    assert!(!super::is_valid_tool_name(""));
}
