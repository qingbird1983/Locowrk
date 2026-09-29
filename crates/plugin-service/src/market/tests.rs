use std::path::{Path, PathBuf};

use semver::Version;

use super::{LocalDirMarket, Market, artifact_file_name};

/// 造假包目录：packaging.md v2 全量 manifest.json + 假库文件。
fn write_package(root: &Path, id: &str, version: &str, description: &str, origin: &str) {
    let dir = root.join(id).join(version);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = format!(
        r#"{{
        "id": "{id}", "version": "{version}", "abi_version": 1, "kind": "tool",
        "display_name": {{"zh-CN": "测试插件", "en-US": "Test Plugin"}},
        "description": {{"zh-CN": "中文描述", "en-US": "{description}"}},
        "icon": "assets/icon.svg",
        "origin": "{origin}",
        "tools": [{{"name": "run", "description": "d", "schema": {{}}, "side_effect": "Pure"}}]
    }}"#
    );
    std::fs::write(dir.join("manifest.json"), manifest).unwrap();
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(
        dir.join("assets/icon.svg"),
        b"<svg xmlns='http://www.w3.org/2000/svg'/>",
    )
    .unwrap();
    std::fs::write(dir.join(artifact_file_name(id)), b"fake").unwrap();
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-market-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn catalog_lists_only_well_formed_versions_sorted() {
    let root = temp_root("catalog");
    write_package(&root, "tool-b", "1.0.0", "b one", "community");
    write_package(&root, "tool-b", "1.1.0", "b two", "community");
    write_package(&root, "tool-a", "0.1.0", "a", "community");
    // 残缺包：缺库文件 → 不该出现在目录里
    let broken = root.join("tool-broken").join("0.9.0");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(
        broken.join("manifest.json"),
        r#"{"id":"tool-broken","version":"0.9.0","description":"x"}"#,
    )
    .unwrap();
    // 版本目录名不是 semver → 忽略
    std::fs::create_dir_all(root.join("tool-b").join("not-a-version")).unwrap();

    let items = LocalDirMarket::new(root).catalog().unwrap();
    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["tool-a", "tool-b"]
    );
    let b = items.iter().find(|i| i.id == "tool-b").unwrap();
    assert_eq!(
        b.versions,
        vec![Version::new(1, 0, 0), Version::new(1, 1, 0)]
    );
    assert_eq!(b.latest().unwrap(), &Version::new(1, 1, 0));
    // 展示投影取 latest 版本的 manifest（packaging.md §5.3）
    assert_eq!(
        b.display
            .description
            .as_ref()
            .unwrap()
            .get("en-US")
            .map(String::as_str),
        Some("b two")
    );
    assert_eq!(
        b.display
            .display_name
            .as_ref()
            .unwrap()
            .get("zh-CN")
            .map(String::as_str),
        Some("测试插件")
    );
    assert!(
        b.display
            .icon
            .as_deref()
            .unwrap_or("")
            .starts_with("data:image/svg+xml;base64,"),
        "图标应编码为 data URL"
    );
}

/// 坏 manifest 的版本目录跳过，不崩整表（packaging.md §11"列表崩"）。
#[test]
fn catalog_skips_broken_manifest_versions() {
    let root = temp_root("broken");
    write_package(&root, "tool-ok", "1.0.0", "ok", "community");
    // manifest 与目录名不一致 → 该版本跳过
    let mismatch = root.join("tool-ok").join("2.0.0");
    std::fs::create_dir_all(&mismatch).unwrap();
    std::fs::write(
        mismatch.join("manifest.json"),
        r#"{"id":"tool-ok","version":"9.9.9","abi_version":1,"kind":"tool","tools":[]}"#,
    )
    .unwrap();
    std::fs::write(mismatch.join(artifact_file_name("tool-ok")), b"fake").unwrap();

    let items = LocalDirMarket::new(root).catalog().unwrap();
    let ok = items.iter().find(|i| i.id == "tool-ok").unwrap();
    assert_eq!(ok.versions, vec![Version::new(1, 0, 0)]);
}

#[test]
fn catalog_on_missing_root_is_empty_not_error() {
    let market = LocalDirMarket::new(temp_root("missing").join("nope"));
    assert!(market.catalog().unwrap().is_empty());
}

#[test]
fn package_validates_existence_and_manifest_consistency() {
    let root = temp_root("package");
    let market = LocalDirMarket::new(root.clone());
    let v = Version::new(1, 2, 0);

    // 不存在的包
    assert!(market.package("tool-x", &v).is_err());

    write_package(&root, "tool-x", "1.2.0", "x", "official");
    let dir = market.package("tool-x", &v).unwrap();
    assert!(dir.join(artifact_file_name("tool-x")).exists());

    // manifest 与目录名不一致 → 拒绝
    let dir = root.join("tool-y").join("2.0.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"id":"tool-y","version":"9.9.9","abi_version":1,"kind":"tool","tools":[]}"#,
    )
    .unwrap();
    std::fs::write(dir.join(artifact_file_name("tool-y")), b"fake").unwrap();
    assert!(market.package("tool-y", &Version::new(2, 0, 0)).is_err());
}
