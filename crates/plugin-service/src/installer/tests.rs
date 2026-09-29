use std::path::{Path, PathBuf};

use semver::Version;

use super::extract;
use crate::market::artifact_file_name;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-installer-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 造假市场包。
fn write_pkg(root: &Path, id: &str, version: &str) {
    let dir = root.join(id).join(version);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        format!(r#"{{"id":"{id}","version":"{version}","description":"d"}}"#),
    )
    .unwrap();
    std::fs::write(dir.join(artifact_file_name(id)), b"artifact-bytes").unwrap();
}

#[test]
fn extract_copies_package_into_install_layout() {
    let market = temp_dir("src");
    let install_root = temp_dir("dst");
    write_pkg(&market, "tool-x", "1.0.0");

    let dst = extract(
        &market.join("tool-x").join("1.0.0"),
        &install_root,
        "tool-x",
        &Version::new(1, 0, 0),
    )
    .unwrap();

    assert_eq!(dst, install_root.join("tool-x").join("1.0.0"));
    assert_eq!(
        std::fs::read(dst.join(artifact_file_name("tool-x"))).unwrap(),
        b"artifact-bytes"
    );
    assert!(dst.join("manifest.json").is_file());
}

#[test]
fn extract_rejects_missing_artifact_or_bad_manifest() {
    let market = temp_dir("bad");
    let install_root = temp_dir("dst2");
    let v = Version::new(0, 1, 0);

    // 缺库文件
    let dir = market.join("tool-a").join("0.1.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"id":"tool-a","version":"0.1.0","description":"d"}"#,
    )
    .unwrap();
    assert!(extract(&dir, &install_root, "tool-a", &v).is_err());

    // manifest 版本与目录名不符
    let dir = market.join("tool-b").join("0.1.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"id":"tool-b","version":"0.2.0","description":"d"}"#,
    )
    .unwrap();
    std::fs::write(dir.join(artifact_file_name("tool-b")), b"x").unwrap();
    assert!(extract(&dir, &install_root, "tool-b", &v).is_err());
}

#[test]
fn extract_over_existing_dir_overwrites_without_deleting() {
    let market = temp_dir("src3");
    let install_root = temp_dir("dst3");
    write_pkg(&market, "tool-c", "3.0.0");

    let dst = extract(
        &market.join("tool-c").join("3.0.0"),
        &install_root,
        "tool-c",
        &Version::new(3, 0, 0),
    )
    .unwrap();
    // 目标里预先放一个市场包没有的文件（如运行期产物），重装不应丢
    std::fs::write(dst.join("runtime-artifact.txt"), b"keep").unwrap();

    extract(
        &market.join("tool-c").join("3.0.0"),
        &install_root,
        "tool-c",
        &Version::new(3, 0, 0),
    )
    .unwrap();
    assert!(dst.join("runtime-artifact.txt").is_file());
}
