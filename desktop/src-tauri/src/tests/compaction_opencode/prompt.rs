//! AC-3（提示词与上游**逐字节**相等）与 AC-4（输出校验 + 提醒一次 + 失败文案）。
//!
//! 逐字来源 = 上游 `packages/core/src/session/compaction.ts:107-185`。

use super::*;

/// 上游的 `SUMMARY_TEMPLATE`（`compaction.ts:107-138` 逐字）。
const UPSTREAM_TEMPLATE: &str = "You MUST use this format for your response (you may omit sections that aren't applicable). Do not include the <template> tags in your response.\n<template>\n## Objective\n- [one or two brief sentences describing what the user is trying to accomplish]\n\n## Requirements\n- [constraints, preferences, requirements, and scope boundaries stated by the user, or \"(none)\"]\n\n## Decisions\n- [decisions already made and why, or \"(none)\"]\n\n## Work State\nBreak the objective into smaller goals and report which are completed, which are being worked on, and which are blocked.\n### Completed\n- [goals that have been completed; otherwise \"(none)\"]\n\n### Active\n- [goals currently being worked on; otherwise \"(none)\"]\n\n### Blocked\n- [anything blocking progress, and why; otherwise \"(none)\"]\n\n## Next Move\n1. [ordered list of next actions, or \"(none)\"]\n\n## Relevant Files\nList the files and directories, other than the current working directory, that another agent would need to open to continue this work. Include at most 15, most important first. Do not list every file that was read or changed. Include paths outside the current working directory when relevant. If none, write \"(none)\".\n- `[file or directory path]`: [brief reason it matters]\n\n## Important Context\n- [facts the next agent cannot continue without and cannot easily find on its own; or \"(none)\"]\n</template>";
/// 上游的 `SUMMARY_RULES`（`compaction.ts:140-146` 逐字）。
const UPSTREAM_RULES: &str = "Rules:\n- Keep each section concise. Use terse, single-line bullets, not prose paragraphs or nested lists.\n- Prefer short references over detailed restatement. It is fine to leave out information the next agent can recover from the code or the files listed above.\n- Preserve exact file paths, symbols, commands, error strings, URLs, and identifiers.\n- Carry forward only user questions or requests that remain unanswered or require further action. Do not repeat ones that newer history has answered or resolved. Preserve exact wording when carrying one forward.\n- Preserve consequential workflow state, including whether changes are uncommitted, committed, pushed, under review, or merged.\n- Do not mention the summary process or that context was compacted.";
/// 共享五段的第一段（`compaction.ts:150` 逐字）。
const UPSTREAM_PREAMBLE: &str = "Summarize only what the user and the assistant said and did. Leave out instructions and setup the assistant was given rather than told by the user: repository conventions, instruction files such as AGENTS.md, and environment details like the session ID. The next agent receives current versions of all of these separately.";
/// 共享五段的倒数第二段（`compaction.ts:153` 逐字）。
const UPSTREAM_NO_TOOLS: &str = "Do not continue the task or call tools.";
/// 共享五段的最后一段（`compaction.ts:154` 逐字）。
const UPSTREAM_ONLY_SUMMARY: &str = "Return only the structured summary in the requested format. Do not include a preamble, explanation, or other commentary.";
/// 首次压缩那一句（`compaction.ts:158` 逐字）。
const UPSTREAM_FIRST: &str = "You MUST summarize the conversation above into a structured summary that will be given to another agent to resume the work.";
/// 更新分支第一句（`compaction.ts:163` 逐字）。
const UPSTREAM_UPDATE_HEAD: &str =
    "Update the existing checkpoint in the conversation above into one consolidated summary.";
/// 更新分支的 legacy 变体（`compaction.ts:171` 逐字）。
const UPSTREAM_UPDATE_LEGACY: &str = "The existing checkpoint was written with an earlier format that recorded far more detail than this one asks for. Rewrite it at the level of detail described below rather than carrying its detail forward. Keep its requirements, decisions, and open questions; they came from earlier conversation with the user.";
/// 更新分支第二段（`compaction.ts:174` 逐字）。
const UPSTREAM_PRECEDENCE: &str = "Newer history always takes precedence over the existing checkpoint. Preserve previous information unless newer history clearly contradicts, supersedes, resolves, or makes it stale. If something is no longer relevant to continuing the work, you may remove it.";
/// 更新分支第三段（`compaction.ts:175` 逐字）。
const UPSTREAM_INCORPORATE: &str = "Incorporate newer requirements, decisions, progress, and context. Reconcile Work State and Next Move: move completed work out of Active, remove resolved blockers and answered questions, and preserve unresolved or pending work.";
/// 更新分支第四段（`compaction.ts:176` 逐字）。
const UPSTREAM_ONLY_SECTIONS: &str = "Return only the updated Markdown sections. Do not reproduce the `<conversation-checkpoint>`, `<summary>`, or `<recent-context>` wrapper tags from the previous checkpoint.";
/// 提醒那一句（`compaction.ts:181-182` `NUDGE` 逐字）。
const UPSTREAM_NUDGE: &str = "The previous response did not fill in the required summary template. Do not call tools. Return the summary as text using the exact section headings from the template.";

/// 共享五段（上游 `compaction.ts:149-155`：五个元素用空行连起来）。
fn shared() -> String {
    [
        UPSTREAM_PREAMBLE,
        UPSTREAM_TEMPLATE,
        UPSTREAM_RULES,
        UPSTREAM_NO_TOOLS,
        UPSTREAM_ONLY_SUMMARY,
    ]
    .join("\n\n")
}

/// **AC-3**：四个提示词变体与上游**逐字节相等**；模板 10 个标题行逐字且逐序。
#[test]
fn the_compaction_prompt_matches_opencode_byte_for_byte() {
    // ① 标题行：10 行（`###` 那三行也算——上游的过滤是 `startsWith("##")`）、
    //    逐字且逐序（上游 `session-compaction.test.ts:210-224` 同款断言）。
    assert_eq!(
        crate::compact::prompt::headings(),
        vec![
            "## Objective",
            "## Requirements",
            "## Decisions",
            "## Work State",
            "### Completed",
            "### Active",
            "### Blocked",
            "## Next Move",
            "## Relevant Files",
            "## Important Context",
        ],
        "模板的标题行必须逐字且逐序（少算 `###` 那三行 = 校验放宽）"
    );
    // ② 首次 / 更新 / 更新+legacy 三个变体：与上游逐字节相等。
    let shared = shared();
    assert_eq!(
        crate::compact::prompt::build_prompt(false, false),
        format!("{UPSTREAM_FIRST}\n\n{shared}"),
        "首次压缩的提示词与上游逐字节相等"
    );
    assert_eq!(
        crate::compact::prompt::build_prompt(true, false),
        format!(
            "{UPSTREAM_UPDATE_HEAD}\n\n{UPSTREAM_PRECEDENCE}\n\n{UPSTREAM_INCORPORATE}\n\n{UPSTREAM_ONLY_SECTIONS}\n\n{shared}"
        ),
        "更新压缩的提示词与上游逐字节相等"
    );
    assert_eq!(
        crate::compact::prompt::build_prompt(true, true),
        format!(
            "{UPSTREAM_UPDATE_HEAD}\n\n{UPSTREAM_UPDATE_LEGACY}\n\n{UPSTREAM_PRECEDENCE}\n\n{UPSTREAM_INCORPORATE}\n\n{UPSTREAM_ONLY_SECTIONS}\n\n{shared}"
        ),
        "legacy 变体与上游逐字节相等"
    );
    // ③ 首次变体**不含** legacy 那一句（上游 `buildPrompt(false, true)` 同款）。
    assert!(
        !crate::compact::prompt::build_prompt(false, true).contains(UPSTREAM_UPDATE_LEGACY),
        "legacy 那句只出现在更新分支里"
    );
    // ④ 提醒那一句。
    assert_eq!(
        crate::compact::prompt::nudge(),
        UPSTREAM_NUDGE,
        "提醒那一句与上游逐字节相等"
    );
}

/// **AC-4**：回复里没有模板标题行 ⇒ 追加**一次**提醒重发；再不过 ⇒ 失败，
/// 且错误文案**逐字**用上游那两句之一。
#[tokio::test]
async fn a_summary_without_the_template_gets_one_nudge_then_fails() {
    let script = Script::new(vec![Reply::Text("就一段没有标题的散文".into())]);
    let (cp, _root) = compactor("compact-ac4", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 4, 200);
    let out = cp.run(&mut s, "ac4", &policy(200_000, 50), &hooks()).await;
    // 两次：一次原样、一次带提醒（上游 `compaction.ts:293-316` 同款）。
    assert_eq!(script.calls(), 2, "提醒只给一次");
    let reqs = script.requests();
    assert_eq!(
        user_text(reqs[1].messages.last().expect("有最后一条")),
        UPSTREAM_NUDGE,
        "第二次发的是那条提醒"
    );
    // 判据名 + 上游那句文案。
    assert_eq!(out.reason(), Some("template-mismatch"), "判据名");
    let crate::compact::Outcome::Failed { detail, .. } = &out else {
        panic!("期望失败，实得 {out:?}")
    };
    assert_eq!(
        detail.as_deref(),
        Some("Compaction summary did not match the required template"),
        "失败文案逐字用上游那一句"
    );
}

/// **AC-4 的另一半**：模型一个字都没吐 ⇒ 判据名是 `no-summary`（上游
/// `compaction.ts:311-313` 的两分支）。
#[tokio::test]
async fn an_empty_reply_is_reported_as_no_summary() {
    let script = Script::new(vec![Reply::Text("   ".into())]);
    let (cp, _root) = compactor("compact-ac4b", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 4, 200);
    let out = cp.run(&mut s, "ac4b", &policy(200_000, 50), &hooks()).await;
    assert_eq!(out.reason(), Some("no-summary"), "空回复 ⇒ no-summary");
    let crate::compact::Outcome::Failed { detail, .. } = &out else {
        panic!("期望失败，实得 {out:?}")
    };
    assert_eq!(
        detail.as_deref(),
        Some("Compaction produced no summary"),
        "失败文案逐字用上游那一句"
    );
}

/// **AC-4 的正面那一半**：回复里有**任一**标题行即通过（OR 关系，不要求十行齐）。
#[tokio::test]
async fn a_reply_with_any_template_heading_passes_the_check() {
    let script = Script::new(vec![Reply::Text("## Important Context\n- 只有一行".into())]);
    let (cp, _root) = compactor("compact-ac4c", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 4, 200);
    let out = cp.run(&mut s, "ac4c", &policy(200_000, 50), &hooks()).await;
    assert_eq!(script.calls(), 1, "过了就不提醒");
    assert!(
        matches!(out, crate::compact::Outcome::Compacted { .. }),
        "任一标题行即通过：{out:?}"
    );
}
