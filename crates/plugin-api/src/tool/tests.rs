//! `tool` 模块单测：serde 形状契约。

use serde_json::json;

use super::{CallId, SideEffect};

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
