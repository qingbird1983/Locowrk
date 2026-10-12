use super::*;

// 族表判据（v2.188 · Q4）：三族六站的**先后与去重**。全部量位置而不是量在场——
// 只断「两段都在」分不清「override 排最后」与「override 排最前把 AGENTS 挤掉」，
// 后者正是被否掉的「同层屏蔽」语义。

/// 空文件**不许占掉族位**：仓库根上放一份空 `AGENTS.md`（编辑器误触、git 冲突残留
/// 都造得出来）时，同族的 `.Locowrk/AGENTS.md` 仍然要读到（v2.188 · Q4 新增的
/// 族内去重带来的新边界）。这条与
/// `missing_files_is_none` 分开：那条管"两站都缺 = 整族缺席"，本条管"缺的是哪一站"。
#[test]
fn empty_file_does_not_claim_its_family_slot() {
    let (data, repo, store) = fixture("empty-slot");
    let locowrk = repo.join(crate::repo_context::DIR);
    std::fs::create_dir_all(&locowrk).unwrap();
    std::fs::write(repo.join("AGENTS.md"), "").unwrap();
    std::fs::write(locowrk.join("AGENTS.md"), "real content").unwrap();
    let body = source(&data, &store, true)
        .load(Some("ws-session"))
        .expect("空文件不算命中，继续试同族下一站");
    assert!(body.contains("real content"), "{body}");
    assert!(
        !body.contains(&format!("Contents of {}", repo.join("AGENTS.md").display())),
        "空的那一站不该留下段头"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// 族间串联：`CLAUDE.md` 不抢 `AGENTS.md` 的位置，排在它后面。
#[test]
fn claude_family_is_appended_after_agents_instead_of_replacing_it() {
    let (data, repo, store) = fixture("claude-append");
    std::fs::write(repo.join("AGENTS.md"), "agents body").unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "claude body").unwrap();
    let body = source(&data, &store, true)
        .load(Some("ws-session"))
        .expect("两段都在");
    assert!(
        body.contains("agents body") && body.contains("claude body"),
        "{body}"
    );
    assert!(
        pos(&body, "agents body") < pos(&body, "claude body"),
        "AGENTS 族在前、CLAUDE 族在后"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// 族内二选一：`CLAUDE.md` 与 `.claude/CLAUDE.md` 并存只取仓库根那一份
/// （同一约定的两个家，双读会把同一份规则注两遍）。
#[test]
fn claude_family_takes_the_root_file_before_the_dot_claude_one() {
    let (data, repo, store) = fixture("claude-fallback");
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(repo.join(".claude").join("CLAUDE.md"), "dot claude").unwrap();
    let src = source(&data, &store, true);
    let body = src.load(Some("ws-session")).expect("回落站命中");
    assert!(body.contains("dot claude"), "根上没有时认 .claude/");

    std::fs::write(repo.join("CLAUDE.md"), "root claude").unwrap();
    let body = source(&data, &store, true)
        .load(Some("ws-session"))
        .expect("根站命中");
    assert!(body.contains("root claude"));
    assert!(!body.contains("dot claude"), "族内先到先得，第二站不再读");
    let _ = std::fs::remove_dir_all(&data);
}

/// override 语义 = **追加不替换**（用户 2026-10-13 裁定）：同层被它覆盖的文件照注，
/// override 只是排在链尾（读到顺序在后 = 权重高）。
#[test]
fn override_lands_last_and_masks_nothing() {
    let (data, repo, store) = fixture("override-last");
    std::fs::write(repo.join("AGENTS.md"), "project rules").unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "vendor rules").unwrap();
    std::fs::write(repo.join("AGENTS.override.md"), "my overrides").unwrap();
    let body = source(&data, &store, true)
        .load(Some("ws-session"))
        .expect("三族都在");
    assert!(
        body.contains("project rules") && body.contains("vendor rules"),
        "同层不被屏蔽"
    );
    assert!(
        pos(&body, "project rules") < pos(&body, "vendor rules")
            && pos(&body, "vendor rules") < pos(&body, "my overrides"),
        "链尾 = override，实际顺序：{body}"
    );
    let _ = std::fs::remove_dir_all(&data);
}
