//! repo_context 单测：引导幂等 / gitignore 追加 / **交接段带真实正文**（S3 刀② 起，
//! 方案稿 §F 那句"新会话一眼见现状"）/ 读不到就整段缺席 / 超预算钳制。
//!
//! ⚠ 这一簇里原先有一条 `build_is_static_reminder_independent_of_files`，它断言的正是
//! 刀② 要**反转**的形状（"与文件状态无关"）。删掉它不是删测试凑绿：那条判据描述的
//! 是 v2.136 那一档，而 v2.136 那一档被 §F 重定 + 刀② 落地取代了；替代它的断言写在
//! 下面（"改了正文输出必须跟着变"是同一格的反面证据）。

use super::{build, ensure_layout, DIR, HANDOFF, HANDOFF_MAX_LINES};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    crate::testutil::unique_dir(&format!("repoctx-{}", tag))
}

/// 写一份交接正文（测试道具）。
fn write_handoff(repo: &Path, body: &str) {
    std::fs::create_dir_all(repo.join(DIR)).unwrap();
    std::fs::write(repo.join(DIR).join(HANDOFF), body).unwrap();
}

#[test]
fn ensure_layout_is_idempotent_and_bootstraps_gitignore() {
    let repo = temp_dir("layout");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();

    ensure_layout(&repo).unwrap();
    let dir = repo.join(DIR);
    assert!(dir.is_dir(), "交接目录建立");
    assert!(
        dir.join("requirements.md").exists(),
        "requirements 模板写入"
    );
    assert!(
        dir.join(HANDOFF).exists(),
        "刀② 起 MEMORY.md 骨架也在这里引导——没有它就没有交接段注入"
    );
    let gi = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(
        gi.contains("target/\n.Locowrk/"),
        "追加一行且补前置换行：{gi:?}"
    );

    // 幂等：再跑不改模板 / 不覆盖人写的 / 不重复追加
    std::fs::write(dir.join("requirements.md"), "# 我的要求\n").unwrap();
    std::fs::write(dir.join(HANDOFF), "# 我的现状\n").unwrap();
    ensure_layout(&repo).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("requirements.md")).unwrap(),
        "# 我的要求\n",
        "人的 requirements 不被覆盖"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(HANDOFF)).unwrap(),
        "# 我的现状\n",
        "人写的现状不被骨架覆盖（骨架只在缺席时写）"
    );
    let gi = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert_eq!(gi.lines().filter(|l| *l == ".Locowrk/").count(), 1);
    let _ = std::fs::remove_dir_all(&repo);
}

/// 交接段 = **文件正文 + 提醒行**：读不到那份文件就整段 `None`（不退回旧的静态提醒），
/// 正文改了输出当场跟着变，journal 那一档不注入。
#[test]
fn build_carries_the_handoff_body_and_is_absent_without_it() {
    let repo = temp_dir("build");
    // 没建过 ⇒ 整段缺席。这一条是"读不到"与"读到空壳"的分界：留一份与文件无关的
    // 提醒会把两种场合混成同一个形状，而 §E 那枚开关要的恰恰是前者可辨。
    assert!(build(&repo).is_none(), "无 MEMORY.md ⇒ 整段缺席");

    write_handoff(&repo, "# 现状\n\n## 当前阶段\n- S3 刀② 在跑\n");
    let a = build(&repo).unwrap();
    assert!(
        a.contains("S3 刀② 在跑"),
        "正文原样进段（这就是 §F 要的\"一眼见现状\"）: {a}"
    );
    assert!(
        a.contains("replace the stale section"),
        "原地替换纪律在场: {a}"
    );
    assert!(a.contains("requirements.md"), "requirements 提醒行在场");
    assert!(a.contains("journal/"), "流水按天封卷的指路在场");
    assert!(
        !a.contains("NOT auto-injected"),
        "旧的\"正文不注入\"自述必须已退役: {a}"
    );
    assert!(
        !a.contains("memory_project_history") && !a.contains("handoff.md"),
        "退役工具名与旧文件名不许再出现在注入面: {a}"
    );

    // 正文改了 ⇒ 输出跟着变（这条就是被删那条"静态无关"判据的反面证据）
    write_handoff(&repo, "# 现状\n\n## 当前阶段\n- 刀② 跑完了\n");
    let b = build(&repo).unwrap();
    assert_ne!(a, b, "人改交接文件必须反映到注入面");
    assert!(b.contains("刀② 跑完了") && !b.contains("在跑"));

    // journal 不注入：写一天的流水，正文里不该出现它
    let day = repo.join(DIR).join("journal");
    std::fs::create_dir_all(&day).unwrap();
    std::fs::write(day.join("2026-10-20.md"), "落了刀①，门禁全绿\n").unwrap();
    let c = build(&repo).unwrap();
    assert!(!c.contains("落了刀①"), "journal 按需读，不进上下文: {c}");
    let _ = std::fs::remove_dir_all(&repo);
}

/// 超预算钳制（§F 的硬顶）：行超即截 + 自描述；人把文件写爆不该变成每轮白烧上下文。
#[test]
fn an_oversized_handoff_is_clamped_and_says_so() {
    let repo = temp_dir("oversized");
    let body = (0..HANDOFF_MAX_LINES + 50)
        .map(|i| format!("- line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    write_handoff(&repo, &body);
    let out = build(&repo).unwrap();
    assert!(out.contains("- line 10"), "预算内的部分照给");
    assert!(
        !out.contains(&format!("- line {}", HANDOFF_MAX_LINES + 10)),
        "超顶的行不许进上下文"
    );
    assert!(
        out.contains("truncated at the handoff budget"),
        "截断必须自描述，否则模型以为现状就这些"
    );
    // 预算内的那份不附截断说明（负对照：没有它，"恒附一句"也能绿）
    write_handoff(&repo, "- 短的一行\n");
    assert!(
        !build(&repo).unwrap().contains("truncated"),
        "没超就不许说超了"
    );
    let _ = std::fs::remove_dir_all(&repo);
}
