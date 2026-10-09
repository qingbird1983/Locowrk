//! repo_context 单测：引导幂等 / gitignore 追加 / 静态提醒段（v2.136
//! 项目侧全提醒制——正文不注入，文本与文件内容无关且跨调用逐字节稳定）。

use super::{build, ensure_layout, DIR};
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

/// 提醒段 = 静态文本：与文件内容无关、跨调用逐字节一致（正文不再注入；
/// 三个文件名与两个节点钩子都在场）。
#[test]
fn build_is_static_reminder_independent_of_files() {
    let repo = temp_dir("build");
    // 无目录也照样给提醒（调用侧会先 ensure_layout 引导）
    let a = build(&repo).unwrap();

    std::fs::create_dir_all(repo.join(DIR)).unwrap();
    std::fs::write(repo.join(DIR).join("requirements.md"), "永远用 pnpm\n").unwrap();
    std::fs::write(repo.join(DIR).join("handoff.md"), "进行到 F2\n").unwrap();
    std::fs::create_dir_all(repo.join(DIR).join("notes")).unwrap();
    std::fs::write(
        repo.join(DIR).join("notes").join("2026-10-05-文件化.md"),
        "状态：进行中\n",
    )
    .unwrap();
    let b = build(&repo).unwrap();

    assert_eq!(a, b, "静态提醒段与文件状态无关");
    assert!(a.contains("requirements.md"), "{a}");
    assert!(a.contains("handoff.md"));
    assert!(a.contains("notes/"));
    assert!(
        a.contains("never as an opening ritual"),
        "read 侧条件触发（开场仪式禁令）在场"
    );
    assert!(
        a.contains("update handoff.md"),
        "状态变化时自行更新的钩子在场"
    );
    assert!(!a.contains("at the START"), "「开场必读」旧钩子已退役");
    assert!(
        !a.contains("do not wait to be asked"),
        "「不等被问」只保留在 write 侧（本段是 read 钩子）"
    );
    assert!(a.contains("NOT auto-injected"), "提醒制语义自述在场");
    assert!(!a.contains("永远用 pnpm"), "正文不注入");
    assert!(!a.contains("2026-10-05"), "notes 清单也不注入");
    let _ = std::fs::remove_dir_all(&repo);
}
