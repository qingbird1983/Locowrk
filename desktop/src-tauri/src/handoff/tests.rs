//! handoff 收束更新纯函数测试：仓库根解析（旁车真源）/ 材料组装 / 产出
//! 净化（UNCHANGED、围栏、截断）。LLM 通路与首建事件由实机走查覆盖
//! （AppLlm 无测试替身，同 title.rs 测试口径）。

use super::*;

// ---- repo_root_of：旁车真源 ----

#[test]
fn repo_root_requires_associated_existing_dir() {
    let dir = temp_dir("handoff-root");
    // 未归属（无旁车）= None
    assert!(crate::workspace::repo_root_of(&dir, "s1").is_none());
    // 归属但目录不存在 = None
    session_store::write_workspace(&dir, "s1", Some("Z:\\definitely\\missing")).unwrap();
    assert!(crate::workspace::repo_root_of(&dir, "s1").is_none());
    // 归属且目录存在 = Some
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    session_store::write_workspace(&dir, "s2", Some(repo.to_str().unwrap())).unwrap();
    assert_eq!(
        crate::workspace::repo_root_of(&dir, "s2").as_deref(),
        Some(repo.as_path())
    );
}

// ---- material：当前内容 + 转录 ----

#[test]
fn material_marks_first_creation_and_keeps_current() {
    let m = material("", "User: hi\nAssistant: hello");
    assert!(m.contains("(empty — first creation)"));
    assert!(m.contains("User: hi"));
    let m = material("# 已有交接\n- 上一步：建仓", "User: next");
    assert!(m.contains("# 已有交接"));
    assert!(!m.contains("(empty"));
}

// ---- sanitize_content：UNCHANGED / 围栏 / 截断 ----

#[test]
fn sanitize_rejects_unchanged_and_empty() {
    assert!(sanitize_content("UNCHANGED").is_none());
    assert!(sanitize_content("unchanged").is_none());
    assert!(sanitize_content("  ").is_none());
    assert!(sanitize_content("").is_none());
}

#[test]
fn sanitize_strips_one_markdown_fence() {
    let raw = "```markdown\n## Current state\n- done\n```\n";
    assert_eq!(
        sanitize_content(raw).as_deref(),
        Some("## Current state\n- done")
    );
    // 无尾围栏（被截断的输出）也要能剥头
    let raw = "```\n## Next steps\n- ship";
    assert_eq!(
        sanitize_content(raw).as_deref(),
        Some("## Next steps\n- ship")
    );
    // 无围栏原样通过
    assert_eq!(
        sanitize_content("## Current state\n- ok").as_deref(),
        Some("## Current state\n- ok")
    );
}

#[test]
fn sanitize_clips_runaway_output() {
    let long = "x".repeat(CONTENT_MAX_CHARS + 100);
    let out = sanitize_content(&long).unwrap();
    assert_eq!(out.chars().count(), CONTENT_MAX_CHARS);
}

// ---- 工具 ----

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lw-handoff-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
