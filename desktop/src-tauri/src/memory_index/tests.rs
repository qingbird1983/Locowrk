//! 项目记忆索引注入（MR 稿 §4.1 · 队列 Q2）判据。
//!
//! 两档数据的**优先关系**是主轴：md 投影（人可编辑的人类层）优先，sqlite 现合成
//! 兜底。注意兜底档**不是理论分支**——2026-10-12 本机实测：`memory/projects/`
//! 下四个项目目录里，`TITY` 根本没有 `MEMORY.md`，另外三个只有
//! `# 记忆索引` 空壳标题。所以"文件在场"与"索引有货"是两件事，必须分开判。

use crate::memory::FactLine;

use super::*;

fn line(key: &str, tags: &str, updated_at: i64) -> FactLine {
    FactLine {
        key: key.into(),
        tags: tags.into(),
        updated_at,
    }
}

/// 剥掉导语后的**内容行**。
///
/// 块的形状是「导语 + 换行 + 每行一条指针」，所以 `strip_prefix` 之后剩的第一个
/// 字符是导语与内容之间那个换行符——它不是内容行。判据若不 trim 就会全体
/// off-by-one（首跑就在这连错三条，是判据的口径漏了，不是实现多写了一行）。
fn body_lines(out: &str) -> Vec<&str> {
    out.strip_prefix(LEAD_IN)
        .expect("导语在最前")
        .trim_start_matches('\n')
        .lines()
        .collect()
}

const DAY: i64 = 86_400;
const NOW: i64 = 1_700_000_000;

// ---------------------------------------------------------------- md 优先档

#[test]
fn md_index_wins_and_its_pointer_lines_are_passed_through() {
    // md 的形状照 `plugin_api::memlayout::refresh_index` 的写出格式（生产构造代码）。
    // 「md 命中时根本不查 sqlite」这条优先关系在 `memory/tests.rs` 的集成判据里钉
    // （那层才同时握着两档数据源）。
    let md = "# 记忆索引\n\n- [build.cmd](build.cmd.md) — pnpm tauri dev\n- [stack](stack.md) — Rust 桌面端\n";
    let out = from_md(Some(md)).expect("md 有指针行");
    assert!(
        out.contains("- [build.cmd](build.cmd.md) — pnpm tauri dev"),
        "{out}"
    );
    assert!(out.contains("- [stack](stack.md)"), "{out}");
    assert!(out.contains(LEAD_IN), "导语在场: {out}");
    assert!(!out.contains(TRUNCATED_NOTE), "两行远不到预算: {out}");
}

#[test]
fn an_empty_shell_md_falls_back_to_the_sqlite_side() {
    // **实测现场那一档**：文件在、只有标题 ⇒ 不许注一个空节，也不许当成"没货"
    let shell = "# 记忆索引\n\n";
    assert!(
        from_md(Some(shell)).is_none(),
        "空壳 md 必须交回调用方走兜底档"
    );
    let out = from_rows(&[line("user.stack", "Rust", NOW)], NOW).expect("兜底合成");
    assert!(out.contains("- user.stack — Rust"), "{out}");
}

#[test]
fn missing_md_is_not_an_error_just_no_priority() {
    assert!(from_md(None).is_none());
    assert!(from_md(Some("")).is_none(), "空文件同档");
}

#[test]
fn pointer_extraction_itself_reports_none_when_the_body_has_no_pointer_line() {
    // 「空壳 md 退兜底」在 `from_md` 之上有**两道**判空：`pointer_lines_from_md` 末尾的
    // `then_some` 与 `finish` 开头的 `is_empty`。**变异实测证明这不是纸面担心**：本批
    // 第一轮各摘其一（另一道留在场），`from_md` 的结果一字未变、两条判据双双跑绿
    // ⇒ 单摘任一道都抓不到。因此这里**直接钉私有函数自己的契约**（doc 明写
    // `None` = 一个指针行都没有），让第一道有独有承重点；第二道的独有点在 rows 档
    // （`no_rows_at_all_produces_no_section`），宿主侧的优先关系由 `memory/tests.rs` 钉。
    assert_eq!(pointer_lines_from_md(Some("# 记忆索引\n\n")), None);
    assert_eq!(pointer_lines_from_md(Some("<!-- 人写的批注 -->\n")), None);
    assert_eq!(pointer_lines_from_md(None), None);
    assert_eq!(
        pointer_lines_from_md(Some("# t\n\n- [a](a.md) — x\n")),
        Some(vec!["- [a](a.md) — x".to_string()]),
        "有指针行时原样透传（不重排不改写）"
    );
}

#[test]
fn headings_blank_lines_and_html_comments_are_stripped() {
    // 人写的批注（HTML 注释）不许进上下文；跨行注释也要吞掉
    let md = "# 记忆索引\n\n<!-- 这是批注\n还有第二行\n-->\n\n\
              - [a](a.md) — 甲\n<!-- 单行批注 -->\n- [b](b.md) — 乙\n";
    let out = from_md(Some(md)).expect("两条指针行");
    assert!(!out.contains("批注"), "注释被剥掉: {out}");
    assert!(!out.contains("# 记忆索引"), "标题不进上下文: {out}");
    assert!(
        out.contains("- [a](a.md)") && out.contains("- [b](b.md)"),
        "{out}"
    );
}

#[test]
fn only_pointer_lines_count_as_index_content() {
    // 既非 `- ` 开头也非标题/注释的散句（人随手写的备忘）不进索引，
    // 否则正文会顺着"备忘"这条路混进 system——那是 §4.1 的硬边界。
    let md = "# 记忆索引\n\n随手写的一句话，不是指针行\n- [a](a.md) — 甲\n";
    let out = from_md(Some(md)).unwrap();
    assert!(!out.contains("随手写的一句话"), "非指针行不进: {out}");
    assert!(out.contains("- [a](a.md)"), "{out}");
}

// ---------------------------------------------------------------- sqlite 兜底档

#[test]
fn synthesized_lines_carry_key_hook_and_the_stale_marker() {
    let rows = [
        line("user.lang", "中文", NOW - 60), // 1 分钟内 = 不标
        line("proj.rule", "别动 lockfile", NOW - 3 * DAY), // >48h = 标
        line("no.tag", "", NOW - 10 * DAY),  // 无 tags 不留空钩子
    ];
    let out = from_rows(&rows, NOW).unwrap();
    assert!(out.contains("- user.lang — 中文"), "{out}");
    assert!(!out.contains("user.lang（"), "未过 48h 不标陈旧: {out}");
    assert!(
        out.contains("- proj.rule — 别动 lockfile（3 days old）"),
        "{out}"
    );
    assert!(out.contains("- no.tag（10 days old）"), "{out}");
}

#[test]
fn no_rows_at_all_produces_no_section() {
    assert!(
        from_rows(&[], NOW).is_none(),
        "项目层空 = 整节缺席（零开销）"
    );
}

#[test]
fn hooks_are_flattened_to_one_line_and_capped() {
    let rows = [line("k", &format!("多\n行\n标签{}", "很".repeat(120)), NOW)];
    let out = from_rows(&rows, NOW).unwrap();
    let lines = body_lines(&out);
    assert_eq!(lines.len(), 1, "一条事实就该是一行: {lines:?}");
    assert!(lines[0].contains('…'), "超 80 字符被截断: {}", lines[0]);
    assert!(
        lines[0].starts_with("- k — 多 行 标签"),
        "换行压成空格: {}",
        lines[0]
    );
}

// ---------------------------------------------------------------- 预算

#[test]
fn line_budget_truncates_with_a_pointing_note() {
    let rows: Vec<FactLine> = (0..MAX_INDEX_LINES + 7)
        .map(|i| line(&format!("k{i}"), "tag", NOW - i as i64))
        .collect();
    let out = from_rows(&rows, NOW).unwrap();
    let lines = body_lines(&out);
    assert_eq!(
        lines.len(),
        MAX_INDEX_LINES + 1,
        "200 条指针行 + 1 条 WARNING，不许静默少列"
    );
    assert!(
        lines.last().unwrap().contains(TRUNCATED_NOTE),
        "{:?}",
        lines.last()
    );
    assert!(out.contains("- k0"), "新→旧的头部要在: {out}");
    assert!(!out.contains("- k206"), "超出的那条不该进来");
}

#[test]
fn char_budget_truncates_before_the_line_budget_when_lines_are_fat() {
    // **要靠长 key 撑行**才会撞字符预算：钩子（tags）被 `one_line` 钳到 80 字符，
    // 第一版我拿 190 字符的 tags 当夹具，实际每行只有 ~90 字符、200 行 = 18k
    // 字符根本不撞线 ⇒ 那是一条空转判据（同族 = 台账「空转对照」）。
    // key 的合法长度上限是 512 字节，200 字符量级的 key 是真实落库形态。
    let wide = "x".repeat(200);
    let rows: Vec<FactLine> = (0..MAX_INDEX_LINES)
        .map(|i| line(&format!("{wide}k{i}"), "t", NOW - i as i64))
        .collect();
    let out = from_rows(&rows, NOW).unwrap();
    let lines = body_lines(&out);
    assert!(
        out.contains(TRUNCATED_NOTE),
        "字符超限也要留指路行：模型必须知道还有更多没进上下文"
    );
    let kept = lines.len() - 1; // 减去那条 WARNING
    assert!(
        kept < MAX_INDEX_LINES,
        "字符预算先撞线，应少于 {MAX_INDEX_LINES} 条指针行，实际 {kept}"
    );
    let chars: usize = lines[..kept].iter().map(|l| l.chars().count() + 1).sum();
    assert!(
        chars <= MAX_INDEX_CHARS,
        "收口后必须真在字符预算内，实际 {chars}"
    );
}

#[test]
fn exactly_at_the_line_budget_no_warning_is_added() {
    // 阳性对照的反面：预算**内**不许出现 WARNING（它一旦出现就该是真截断）
    let rows: Vec<FactLine> = (0..MAX_INDEX_LINES)
        .map(|i| line(&format!("k{i}"), "t", NOW - i as i64))
        .collect();
    let out = from_rows(&rows, NOW).unwrap();
    assert!(!out.contains(TRUNCATED_NOTE), "刚好 200 条不算截断");
    assert_eq!(body_lines(&out).len(), MAX_INDEX_LINES);
}

// ------------------------------------------ md 支路的截断标记与预算（S1 刀②b-3）

/// 写侧已经把那句截断说明**写进文件**了，注入通路必须原样把它带给模型。
/// 少了这一半，形状就是"人打开文件夹看到『还有更多没进来』，而模型拿到的是一份
/// 看起来完整的索引"——比不钳更糟，因为它静默。
#[test]
fn a_truncated_index_file_keeps_the_warning_for_the_model() {
    let md = format!("# 记忆索引\n\n- [a](a.md) — 甲\n- [b](b.md) — 乙\n\n{TRUNCATED_NOTE}\n");
    let out = from_md(Some(&md)).expect("两条指针行");
    let lines = body_lines(&out);
    assert_eq!(lines.len(), 3, "两条指针 + 那句截断说明：{lines:?}");
    assert_eq!(
        *lines.last().unwrap(),
        TRUNCATED_NOTE,
        "远不到预算也要留着——**文件说截断了就是截断了**，注入侧不参与判断"
    );
}

/// 人把 `MEMORY.md` 手工编辑成超出预算的形状时，注入通路仍然要钳。
/// 预算保护站在通路上不是冗余：真源 = 文件，而文件正是**人会去改**的那个东西。
#[test]
fn the_budget_still_bites_when_a_human_grows_the_index() {
    let mut md = String::from("# 记忆索引\n\n");
    for i in 0..MAX_INDEX_LINES + 40 {
        md.push_str(&format!("- [k{i}](k{i}.md) — 钩\n"));
    }
    let out = from_md(Some(&md)).expect("指针行充足");
    let lines = body_lines(&out);
    assert_eq!(
        lines.len(),
        MAX_INDEX_LINES + 1,
        "钳到 {MAX_INDEX_LINES} 条 + 1 条截断说明，实际 {}",
        lines.len()
    );
    assert!(out.contains(TRUNCATED_NOTE), "钳过必须自描述");
    // 断言里用裸 key：写成 `- k239` 永远命中不到（文件里的形状是 `- [k239](…)`），
    // 那就是一条空转判据。
    assert!(!out.contains("k239"), "超出的那条不该进来");
}

/// 两支共用**同一句**截断说明（刀②b-3 要消的那处暂时状态）：以前文件里写一句给人看、
/// 注入里另写一句给模型看 = 同一件事两个出处。这条断言防的是有人重新自造一份文案。
#[test]
fn both_sides_share_the_one_truncation_sentence() {
    assert_eq!(TRUNCATED_NOTE, plugin_api::memlayout::INDEX_TRUNCATED_NOTE);
}
