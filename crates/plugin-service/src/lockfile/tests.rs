use std::path::PathBuf;

use semver::Version;

use super::LockFile;

#[test]
fn missing_file_loads_as_empty_lockfile() {
    let dir = tempfile_dir();
    let path = dir.join("lockfile.json");
    let lf = LockFile::load(&path).unwrap();
    assert!(lf.plugins.is_empty());
    assert_eq!(lf.schema_version, super::LOCKFILE_SCHEMA_VERSION);
}

#[test]
fn roundtrip_preserves_entries_and_tombstones() {
    let dir = tempfile_dir();
    let path = dir.join("nested").join("lockfile.json");

    let mut lf = LockFile::load(&path).unwrap();
    lf.add(
        "tool-a",
        Version::new(1, 0, 0),
        7,
        super::InstallSource::Market,
    );
    lf.add(
        "tool-b",
        Version::new(0, 2, 1),
        9,
        super::InstallSource::Local,
    );
    lf.push_pending_delete(PathBuf::from("C:/locked/dir"));
    lf.push_pending_delete(PathBuf::from("C:/locked/dir")); // 去重
    lf.save(&path).unwrap();

    let back = LockFile::load(&path).unwrap();
    assert_eq!(back.plugins.len(), 2);
    let a = back.entry("tool-a").unwrap();
    assert_eq!(a.version, Version::new(1, 0, 0));
    assert_eq!(a.effect_ids, vec![7]);
    assert!(a.enabled);
    assert_eq!(back.pending_delete, vec![PathBuf::from("C:/locked/dir")]);
}

#[test]
fn update_version_and_set_enabled_require_existing_entry() {
    let mut lf = LockFile::default();
    assert!(lf.update_version("ghost", &Version::new(1, 0, 0)).is_err());
    assert!(lf.set_enabled("ghost", false).is_err());

    lf.add(
        "tool-a",
        Version::new(1, 0, 0),
        1,
        super::InstallSource::Market,
    );
    lf.update_version("tool-a", &Version::new(2, 0, 0)).unwrap();
    lf.set_enabled("tool-a", false).unwrap();
    let e = lf.entry("tool-a").unwrap();
    assert_eq!(e.version, Version::new(2, 0, 0));
    assert!(!e.enabled);
}

#[test]
fn remove_drops_entry_and_clear_pending_only_matches() {
    let mut lf = LockFile::default();
    lf.add(
        "tool-a",
        Version::new(1, 0, 0),
        1,
        super::InstallSource::Market,
    );
    let gone = lf.remove("tool-a").unwrap();
    assert_eq!(gone.id, "tool-a");
    assert!(lf.entry("tool-a").is_none());
    // 原此处还断过 `clear_pending_delete`（与 startup_reconcile 同语义的第二
    // 份实现，全仓无生产调用点）——审计 M-22/D6 已随方法一并删除；tombstone
    // 的追加/去重断言在本文件 push 测试里，清理路径断言在 service 生命周期测
    // 试里（`service/tests/lifecycle.rs`）。
}

fn tempfile_dir() -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "locowrk-lockfile-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    base
}
