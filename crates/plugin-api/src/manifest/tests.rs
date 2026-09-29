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
        display_name: None,
        description: None,
        icon: None,
        author: None,
        origin: PluginOrigin::Community,
        homepage: None,
        license: None,
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

/// v2 纪律：未设置的 v2 字段不得出现在 describe JSON 里（旧宿主照常解析）。
/// 带引号匹配，避免撞上工具自带的 `description` 字段。
#[test]
fn v2_fields_absent_from_describe_when_unset() {
    let cstr = sample().to_describe_json();
    let text = std::str::from_utf8(cstr.to_bytes()).unwrap();
    for key in [
        "\"display_name\"",
        "\"icon\"",
        "\"author\"",
        "\"origin\"",
        "\"license\"",
    ] {
        assert!(!text.contains(key), "{key} must be omitted: {text}");
    }
}

/// packaging.md §9 纪律：v1 极简 manifest（无任何 v2 字段）照常解析。
#[test]
fn v1_manifest_without_new_fields_parses() {
    let m = PluginManifest::from_json(
        br#"{"id":"tool-old","version":"0.9.0","abi_version":1,"kind":"tool",
             "tools":[{"name":"old","description":"d","schema":{},"side_effect":"Pure"}]}"#,
    )
    .expect("v1 manifest parses");
    assert_eq!(m.origin, PluginOrigin::Community, "缺省一律 community");
    assert!(m.display_name.is_none() && m.description.is_none() && m.icon.is_none());
    assert_eq!(m.tools[0].side_effect, SideEffect::Pure, "纯字符串双形态");
}

/// packaging.md §2.1 完整示例：多语言 + 图标 + 作者 + 官方来源 + 工具级
/// display_name 与 manifest 级 reversible（未知字段必须被忽略）。
#[test]
fn packaging_doc_example_parses() {
    let m = PluginManifest::from_json(crate::tests::DOC_EXAMPLE.as_bytes())
        .expect("doc example parses");
    assert_eq!(m.origin, PluginOrigin::Official);
    assert_eq!(
        m.display_name.as_ref().unwrap().pick("zh-CN"),
        Some("大小写转换")
    );
    assert_eq!(
        m.display_name.as_ref().unwrap().pick("ja-JP"),
        Some("シャウト")
    );
    assert_eq!(
        m.description.as_ref().unwrap().pick("zh-CN"),
        Some("把文本转换成大写输出")
    );
    assert_eq!(m.icon.as_deref(), Some("assets/icon.svg"));
    assert_eq!(m.author.as_ref().unwrap().name, "Locowrk Team");
    assert_eq!(m.license.as_deref(), Some("MIT"));
    // 文档式 manifest 的工具级 display_name / manifest 级 reversible：忽略不报错
    assert_eq!(m.tools.len(), 1);
}

/// SideEffect 双形态（DESIGN §14 v2.10 对齐决定）。
#[test]
fn side_effect_accepts_string_and_tagged_forms() {
    for raw in ["Pure", "pure", "PURE"] {
        assert_eq!(
            serde_json::from_str::<SideEffect>(&format!(r#""{raw}""#)).unwrap(),
            SideEffect::Pure
        );
    }
    assert_eq!(
        serde_json::from_str::<SideEffect>(r#"{"kind":"irreversible"}"#).unwrap(),
        SideEffect::Irreversible
    );
    // 补偿必须走 tagged（undo_tool 只有对象形态带得动）
    assert!(serde_json::from_str::<SideEffect>(r#""Compensable""#).is_err());
    assert!(serde_json::from_str::<SideEffect>(r#"{"kind":"compensable"}"#).is_err());
    assert_eq!(
        serde_json::from_str::<SideEffect>(r#"{"kind":"compensable","undo_tool":"undo_x"}"#)
            .unwrap(),
        SideEffect::Compensable {
            undo_tool: "undo_x".into()
        }
    );
    // 序列化维持现行 tagged snake_case（ABI 边界不破坏旧宿主）
    assert_eq!(
        serde_json::to_string(&SideEffect::Pure).unwrap(),
        r#"{"kind":"pure"}"#
    );
}

/// description 双形态 + LocalizedString 回退链（packaging.md §2.3）。
#[test]
fn description_dual_form_and_pick_fallback_chain() {
    let s = PluginManifest::from_json(
        br#"{"id":"t","version":"1.0.0","abi_version":1,"kind":"tool",
             "tools":[],"description":"plain old string"}"#,
    )
    .unwrap();
    assert_eq!(
        s.description.as_ref().unwrap().pick("zh-CN"),
        Some("plain old string")
    );

    let l = LocalizedString(BTreeMap::from([
        ("en-US".into(), "Shout".into()),
        ("zh-TW".into(), "繁體".into()),
        ("fr-FR".into(), "Crie".into()),
    ]));
    assert_eq!(l.pick("zh-CN"), Some("繁體"), "主语言回退（zh-CN → zh-TW）");
    assert_eq!(l.pick("en-GB"), Some("Shout"), "英语回退");
    assert_eq!(l.pick("de-DE"), Some("Shout"), "en-US 先于任意回退");
    // 没有 en-US 时才落到"任意"（字典序最小 key）
    let no_en = LocalizedString(BTreeMap::from([
        ("zh-TW".into(), "繁體".into()),
        ("fr-FR".into(), "Crie".into()),
    ]));
    assert_eq!(
        no_en.pick("de-DE"),
        Some("Crie"),
        "任意回退 = 字典序最小 key"
    );
    assert!(LocalizedString::default().pick("en-US").is_none());
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
