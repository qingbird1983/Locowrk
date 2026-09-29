use super::*;
use crate::tool::{SideEffect, ToolSpec};

fn sample() -> PluginManifest {
    PluginManifest {
        id: "tool-echo".into(),
        version: "1.3.0".into(),
        abi_version: crate::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![ToolSpec {
            name: "echo".into(),
            description: "echoes text".into(),
            schema: serde_json::json!({"type": "object"}),
            side_effect: SideEffect::Pure,
        }],
    }
}

#[test]
fn describe_json_roundtrip() {
    let cstr = sample().to_describe_json();
    assert_eq!(cstr.to_bytes().last(), Some(&b'}'), "json ends with brace");
    let parsed = PluginManifest::from_json(cstr.to_bytes()).expect("parse back");
    assert_eq!(parsed.id, "tool-echo");
    assert_eq!(parsed.version, "1.3.0");
    assert_eq!(parsed.kind, PluginKind::Tool);
    assert_eq!(parsed.tools.len(), 1);
    assert_eq!(parsed.tools[0].side_effect, SideEffect::Pure);
}

#[test]
fn invalid_json_is_reported_not_panicked() {
    assert!(matches!(
        PluginManifest::from_json(b"{not json"),
        Err(ManifestError::Json(_))
    ));
    assert!(matches!(
        PluginManifest::from_json(b"\xff\xfe"),
        Err(ManifestError::Utf8(_))
    ));
}
