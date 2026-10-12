use super::*;

// 既有判据（v2.139 建簇时写下）：两段拼装 / 回落链 / 缺文件 / 快照锁字节 /
// 超限截断 / 开关关。**本批一条断言都没改**——它们仍然绿就是
// 「可调项缺省 = 改前那颗写死值」的等价性证据。

#[test]
fn loads_global_and_project_sections() {
    let (data, repo, store) = fixture("both");
    std::fs::write(data.join("AGENTS.md"), "always answer in English").unwrap();
    std::fs::write(repo.join("AGENTS.md"), "use pnpm").unwrap();

    let body = source(&data, &store, true)
        .load(Some("ws-session"))
        .expect("两段都在");
    assert!(body.starts_with("# agentsMd\n"), "{body}");
    assert!(
        body.contains("IMPORTANT: These instructions OVERRIDE"),
        "{body}"
    );
    assert!(body.contains(&format!("Contents of {}", data.join("AGENTS.md").display())));
    assert!(body.contains("always answer in English"));
    assert!(body.contains("use pnpm"));
    let _ = std::fs::remove_dir_all(&data);
}

/// 项目根回落链：AGENTS.md 优先；只有 .Locowrk/AGENTS.md 时认后者；
/// 两者并存 = 前者赢。
#[test]
fn project_fallback_chain() {
    let (data, repo, store) = fixture("fallback");
    let locowrk = repo.join(crate::repo_context::DIR);
    std::fs::create_dir_all(&locowrk).unwrap();
    std::fs::write(locowrk.join("AGENTS.md"), "from .Locowrk").unwrap();

    let src = source(&data, &store, true);
    let body = src.load(Some("ws-session")).expect("回落段在场");
    assert!(body.contains("from .Locowrk"), "{body}");

    // AGENTS.md 出现即优先（回落链第一站）
    std::fs::write(repo.join("AGENTS.md"), "from root").unwrap();
    let src2 = source(&data, &store, true);
    let body = src2.load(Some("ws-session")).expect("首选项在场");
    assert!(body.contains("from root"), "{body}");
    assert!(!body.contains("from .Locowrk"), "回落不再读");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn missing_files_is_none() {
    let (data, _repo, store) = fixture("missing");
    assert!(source(&data, &store, true)
        .load(Some("ws-session"))
        .is_none());
    assert!(source(&data, &store, true).load(None).is_none());
    let _ = std::fs::remove_dir_all(&data);
}

/// 快照锁字节（设计稿 §3.3）：同会话首查定格，人改文件当前会话不变；
/// 新会话 = 新快照。
#[test]
fn snapshot_locks_per_session() {
    let (data, repo, store) = fixture("snapshot");
    let path = repo.join("AGENTS.md");
    std::fs::write(&path, "v1 rules").unwrap();
    let src = source(&data, &store, true);
    let first = src.load(Some("ws-session")).unwrap();
    std::fs::write(&path, "v2 rules").unwrap();
    assert_eq!(
        src.load(Some("ws-session")).unwrap(),
        first,
        "当前会话吃快照"
    );
    assert!(
        src.load(Some("ws-fresh")).unwrap().contains("v2 rules"),
        "新会话重读（ws- 前缀 = 同仓库归属）"
    );
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn oversize_file_truncated_with_marker() {
    let (data, repo, store) = fixture("cap");
    let big = "x".repeat(101 * 1024);
    std::fs::write(repo.join("AGENTS.md"), &big).unwrap();
    let body = source(&data, &store, true)
        .load(Some("ws-session"))
        .unwrap();
    assert!(body.contains("[File truncated"), "{body}");
    // 精确口径：x 恰 = 截断值 + 头部句 "exactly" 的 1 个（路径与标记句
    // 均不含 x）——多一个都说明钳制失守。
    assert_eq!(
        body.matches('x').count(),
        100 * 1024 + 1,
        "正文钳在 100KB 内（+1 = agentsMd 头部 exactly 的 x）"
    );
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn disabled_switch_is_none() {
    let (data, repo, store) = fixture("disabled");
    std::fs::write(repo.join("AGENTS.md"), "rules").unwrap();
    assert!(source(&data, &store, false)
        .load(Some("ws-session"))
        .is_none());
    let _ = std::fs::remove_dir_all(&data);
}
