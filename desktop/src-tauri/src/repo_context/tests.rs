//! repo_context 单测：引导幂等 / gitignore 追加 / 指纹 / 注入段内容。

use super::{build, ensure_layout, fingerprint, DIR};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "locowrk-repoctx-{tag}-{nanos}-{}",
        std::process::id()
    ))
}

#[test]
fn ensure_layout_is_idempotent_and_bootstraps_gitignore() {
    let repo = temp_dir("layout");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();

    ensure_layout(&repo).unwrap();
    let dir = repo.join(DIR);
    assert!(dir.is_dir(), "交接目录建立");
    assert!(dir.join("requirements.md").exists(), "模板写入");
    let gi = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(
        gi.contains("target/\n.Locowrk/"),
        "追加一行且补前置换行：{gi:?}"
    );

    // 幂等：再跑不改模板 / 不重复追加
    std::fs::write(dir.join("requirements.md"), "# 我的要求\n").unwrap();
    ensure_layout(&repo).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("requirements.md")).unwrap(),
        "# 我的要求\n",
        "人的 requirements 不被覆盖"
    );
    let gi = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert_eq!(gi.lines().filter(|l| *l == ".Locowrk/").count(), 1);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn build_reflects_files_and_fingerprint_tracks_state() {
    let repo = temp_dir("build");
    // 无目录 = None（未归属 / 未引导零注入）
    assert_eq!(build(&repo), None);
    assert_eq!(fingerprint(&repo), "");

    // 有目录但内容全空 → 提示段（用户清空了 requirements / 无 handoff）
    std::fs::create_dir_all(repo.join(DIR)).unwrap();
    let hint = build(&repo).unwrap();
    assert!(hint.contains("requirements.md awaits"), "{hint}");

    // 引导 → 模板就位 → 完整段
    ensure_layout(&repo).unwrap();
    let dir = repo.join(DIR);
    std::fs::write(dir.join("requirements.md"), "永远用 pnpm\n").unwrap();
    std::fs::write(dir.join("handoff.md"), "进行到 F2\n").unwrap();
    std::fs::create_dir_all(dir.join("notes")).unwrap();
    std::fs::write(
        dir.join("notes").join("2026-10-05-文件化.md"),
        "状态：进行中\n",
    )
    .unwrap();
    let section = build(&repo).unwrap();
    assert!(section.contains("--- requirements.md ---"), "{section}");
    assert!(section.contains("永远用 pnpm"));
    assert!(section.contains("--- handoff.md ---"));
    assert!(section.contains("- 2026-10-05-文件化.md"));
    assert!(!section.contains("状态：进行中"), "notes 只给清单不给全文");

    // 指纹：内容变化 → 指纹变化（mtime 键）；空目录指纹 = requirements+hoff 两项
    let fp1 = fingerprint(&repo);
    assert!(fp1.contains("requirements.md:"), "{fp1}");
    assert!(fp1.contains("n:2026-10-05-文件化.md:"), "notes 清单进指纹");
    std::thread::sleep(std::time::Duration::from_millis(25));
    std::fs::write(dir.join("handoff.md"), "进行到 F3\n").unwrap();
    let fp2 = fingerprint(&repo);
    assert_ne!(fp1, fp2, "人改文件 = 显式变更（缓存键变）");
    let _ = std::fs::remove_dir_all(&repo);
}
