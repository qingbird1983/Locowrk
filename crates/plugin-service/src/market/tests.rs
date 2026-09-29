use std::path::{Path, PathBuf};

use semver::Version;

use super::{LocalDirMarket, Market, PackageManifest, artifact_file_name};

/// 造一个假包目录：manifest.json + 假库文件。
fn write_package(root: &Path, id: &str, version: &str, description: &str) {
    let dir = root.join(id).join(version);
    std::fs::create_dir_all(&dir).unwrap();
    let m = PackageManifest {
        id: id.to_string(),
        version: version.to_string(),
        description: description.to_string(),
    };
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&m).unwrap(),
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
    write_package(&root, "tool-b", "1.0.0", "b one");
    write_package(&root, "tool-b", "1.1.0", "b two");
    write_package(&root, "tool-a", "0.1.0", "a");
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
    assert_eq!(b.description, "b one"); // 首个可读 manifest 的描述
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

    write_package(&root, "tool-x", "1.2.0", "x");
    let dir = market.package("tool-x", &v).unwrap();
    assert!(dir.join(artifact_file_name("tool-x")).exists());

    // manifest 与目录名不一致 → 拒绝
    let dir = root.join("tool-y").join("2.0.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"id":"tool-y","version":"9.9.9","description":"y"}"#,
    )
    .unwrap();
    std::fs::write(dir.join(artifact_file_name("tool-y")), b"fake").unwrap();
    assert!(market.package("tool-y", &Version::new(2, 0, 0)).is_err());
}
