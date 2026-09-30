//! builtin 解析的单测（纯文件系统操作，不加载真实 DLL）。

use super::*;

#[test]
fn resolve_latest_picks_max_semver_dir() {
    let root = std::env::temp_dir().join(format!(
        "locowrk-builtin-latest-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for v in ["1.0.0", "1.10.0", "1.2.0", "not-a-version"] {
        std::fs::create_dir_all(root.join(v)).unwrap();
    }
    let (v, dir) = resolve_latest(&root).unwrap();
    assert_eq!(v.to_string(), "1.10.0");
    assert_eq!(dir, root.join("1.10.0"));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn resolve_latest_missing_dir_is_none() {
    assert_eq!(resolve_latest(Path::new("/definitely/not/here")), None);
}

#[test]
fn find_artifact_prefers_conventional_then_wasm_then_single_scan() {
    let root = std::env::temp_dir().join(format!(
        "locowrk-builtin-artifact-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dir = root.join("1.0.0");
    std::fs::create_dir_all(&dir).unwrap();

    // 零个工件
    assert_eq!(find_artifact(&dir, "tool-x"), None);

    // 多个散件：歧义拒绝
    std::fs::write(dir.join("a.dll"), b"x").unwrap();
    std::fs::write(dir.join("b.dll"), b"x").unwrap();
    assert_eq!(find_artifact(&dir, "tool-x"), None);

    // 恰好一个散件：采用
    std::fs::remove_file(dir.join("b.dll")).unwrap();
    assert_eq!(
        find_artifact(&dir, "tool-x"),
        Some(dir.join("a.dll")),
        "single scan candidate wins when no conventional name"
    );

    // 约定 wasm 名优先于散件扫描
    std::fs::write(dir.join("tool_x.wasm"), b"x").unwrap();
    assert_eq!(find_artifact(&dir, "tool-x"), Some(dir.join("tool_x.wasm")));

    // 约定 native 名最高优先
    std::fs::write(dir.join(dll_name("tool-x")), b"x").unwrap();
    assert_eq!(
        find_artifact(&dir, "tool-x"),
        Some(dir.join(dll_name("tool-x")))
    );

    std::fs::remove_dir_all(&root).unwrap();
}
