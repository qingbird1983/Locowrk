use std::collections::BTreeMap;
use std::path::PathBuf;

use super::read_display;
use plugin_host::IconLoader;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-display-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const SVG: &[u8] = br#"<svg viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg"/>"#;

fn manifest_json(icon: Option<&str>, tools_side_effects: &[&str]) -> String {
    let tools: Vec<String> = tools_side_effects
        .iter()
        .map(|se| {
            format!(
                r#"{{"name":"t_{se}","description":"d","schema":{{}},"side_effect":{{"kind":"{se}"{}}}}}"#,
                if *se == "compensable" { r#","undo_tool":"undo""# } else { "" }
            )
        })
        .collect();
    let icon_field = icon
        .map(|i| format!(r#""icon":"{i}","#))
        .unwrap_or_default();
    format!(
        r#"{{
        "id": "tool-x", "version": "1.0.0", "abi_version": 1, "kind": "tool",
        "display_name": {{"zh-CN": "文件工具", "en-US": "File Tools"}},
        "description": {{"zh-CN": "只读文件操作", "en-US": "Read-only file ops"}},
        {icon_field}
        "author": {{"name": "Locowrk Team"}},
        "origin": "official",
        "license": "MIT",
        "tools": [{}]
    }}"#,
        tools.join(",")
    )
}

#[test]
fn full_manifest_projects_display_with_icon() {
    let dir = temp_dir("full");
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("assets/icon.svg"), SVG).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        manifest_json(Some("assets/icon.svg"), &["pure"]),
    )
    .unwrap();

    let d = read_display(&dir, &IconLoader::new());
    assert_eq!(d.origin, plugin_api::manifest::PluginOrigin::Official);
    assert_eq!(
        d.display_name
            .as_ref()
            .unwrap()
            .get("zh-CN")
            .map(String::as_str),
        Some("文件工具")
    );
    assert_eq!(
        d.description
            .as_ref()
            .unwrap()
            .get("en-US")
            .map(String::as_str),
        Some("Read-only file ops")
    );
    assert!(
        d.icon
            .as_deref()
            .unwrap_or("")
            .starts_with("data:image/svg+xml;base64,")
    );
    assert_eq!(d.author.as_ref().unwrap().name, "Locowrk Team");
    assert_eq!(d.license.as_deref(), Some("MIT"));
    assert_eq!(d.side_effects, vec!["pure"]);
}

/// Fallback 矩阵（§7）：manifest 缺失 / 损坏 → 缺省投影（community）；
/// 图标缺失 → icon None 但其余字段完好。
#[test]
fn fallback_matrix_degrades_without_crashing() {
    let icons = IconLoader::new();

    // 目录不存在
    assert_eq!(
        read_display(&temp_dir("nope").join("ghost"), &icons),
        super::PluginDisplay::default()
    );

    // manifest.json 是坏 JSON → 缺省投影，不 panic
    let dir = temp_dir("broken");
    std::fs::write(dir.join("manifest.json"), b"{not json").unwrap();
    assert_eq!(read_display(&dir, &icons), super::PluginDisplay::default());

    // 图标文件缺失：其余字段照常投影
    let dir = temp_dir("noicon");
    std::fs::write(
        dir.join("manifest.json"),
        manifest_json(Some("assets/gone.svg"), &["pure"]),
    )
    .unwrap();
    let d = read_display(&dir, &icons);
    assert!(d.icon.is_none());
    assert_eq!(d.origin, plugin_api::manifest::PluginOrigin::Official);
}

/// 副作用去重且按 pure → compensable → irreversible 固定序（多工具乱序
/// 声明也归一）。
#[test]
fn side_effects_dedup_in_fixed_order() {
    let dir = temp_dir("kinds");
    std::fs::write(
        dir.join("manifest.json"),
        manifest_json(None, &["irreversible", "pure", "compensable", "pure"]),
    )
    .unwrap();
    let d = read_display(&dir, &IconLoader::new());
    assert_eq!(d.side_effects, vec!["pure", "compensable", "irreversible"]);
}

#[test]
fn map_keys_serialize_sorted_for_stable_wire() {
    let m = plugin_api::manifest::LocalizedString(BTreeMap::from([
        ("zh-CN".to_owned(), "中".to_owned()),
        ("en-US".to_owned(), "en".to_owned()),
    ]));
    let json = serde_json::to_string(&m).unwrap();
    assert_eq!(json, r#"{"en-US":"en","zh-CN":"中"}"#);
}

/// Fallback 矩阵 §7 行 1/3/4/9：v1 极简 manifest（无任何 v2 展示字段）
/// → 投影全 None（前端回退 id / 首字母 / 不渲染），origin 缺省 community。
#[test]
fn v1_minimal_manifest_projects_all_optional_fields_none() {
    let dir = temp_dir("v1bare");
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"id":"tool-bare","version":"1.0.0","abi_version":1,"kind":"tool",
            "tools":[{"name":"t","description":"d","schema":{},"side_effect":"Pure"}]}"#,
    )
    .unwrap();

    let d = read_display(&dir, &IconLoader::new());
    assert!(d.display_name.is_none(), "display_name 缺失 → 前端显示 id");
    assert!(d.description.is_none(), "description 缺失 → 不渲染描述区");
    assert!(d.icon.is_none(), "icon 缺失 → 前端首字母兜底");
    assert!(
        d.author.is_none() && d.homepage.is_none() && d.license.is_none(),
        "author/homepage/license 缺失 → 对应信息不渲染"
    );
    assert_eq!(d.origin, plugin_api::manifest::PluginOrigin::Community);
    assert_eq!(d.side_effects, vec!["pure"]);
}
