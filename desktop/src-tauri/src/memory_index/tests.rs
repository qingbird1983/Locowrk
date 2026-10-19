//! 项目记忆索引注入（MR 稿 §4.1 · 队列 Q2）判据。
//!
//! 数据源**只剩一档**（S1 刀②c-1）：`MEMORY.md` 里的指针行。原先那档"人类层没接线
//! 才由 sqlite 现合成"连同 `from_rows` / `MemoryStore::index_rows` 一起退役了，所以
//! 本文件的夹具全部改成**照 `refresh_index` 的写出格式造 md**（生产构造代码是唯一依据）。
//! 注意兜底档**不是被我"顺手删掉"的**：2026-10-12 本机实测——`memory/projects/` 下四个
//! 项目目录里 `TITY` 根本没有 `MEMORY.md`，另外三个只有 `# 记忆索引` 空壳标题。真源 =
//! 文件之后，那个形态的意思就是"这几个项目没记过"，`a_shell_index_file…`（宿主集成档，
//! 在 `memory/tests/md_read.rs`）钉的就是这一句。

use super::*;

/// 索引 md 的形状**照生产构造代码**造：`refresh_index` 写的是
/// `- [<文件名主干>](<主干>.md) — <description 单行>`。
fn index_md(pointers: &[&str]) -> String {
    format!("# 记忆索引\n\n{}\n", pointers.join("\n"))
}
fn pointer(stem: &str, hook: &str) -> String {
    format!("- [{stem}]({stem}.md) — {hook}")
}
/// 陈旧标注的输入（`文件名主干 → updated_at`）。
fn ages(pairs: &[(&str, i64)]) -> HashMap<String, i64> {
    pairs.iter().map(|(s, at)| (s.to_string(), *at)).collect()
}

/// 剥掉导语后的**内容行**。
///
/// 块的形状是「导语 + 换行 + 每行一条指针」，所以 `strip_prefix` 之后剩的第一个
/// 字符是导语与内容之间那个换行符——它不是内容行。判据若不 trim 就会全体
/// off-by-one（首跑就在这连错三条，是判据的口径漏了，不是实现多写了一行）。
fn body_lines(out: &str) -> Vec<&str> {
    out.strip_prefix(LEAD_IN_PROJECT)
        .expect("导语在最前")
        .trim_start_matches('\n')
        .lines()
        .collect()
}

const DAY: i64 = 86_400;
const NOW: i64 = 1_700_000_000;

// ---------------------------------------------------------------- 唯一一档：md 投影

#[test]
fn pointer_lines_from_the_index_file_are_passed_through_verbatim() {
    let md = index_md(&[
        &pointer("build.cmd", "pnpm tauri dev"),
        &pointer("stack", "Rust 桌面端"),
    ]);
    let out = from_md(Some(&md), &ages(&[]), NOW, LEAD_IN_PROJECT).expect("md 有指针行");
    assert!(
        out.contains("- [build.cmd](build.cmd.md) — pnpm tauri dev"),
        "{out}"
    );
    assert!(out.contains("- [stack](stack.md)"), "{out}");
    assert!(out.contains(LEAD_IN_PROJECT), "导语在场: {out}");
    assert!(!out.contains(TRUNCATED_NOTE), "两行远不到预算: {out}");
}

/// **实测现场那一档**：文件在、只有 `# 记忆索引` 标题 ⇒ 整节缺席。
/// 刀②b-3 之前这里会退到库里现合成，刀②c-1 之后**没有可退的地方**了：
/// "文件里没指针行"就是"这个项目没记过"。宿主侧同一件事的集成证据在
/// `memory/tests/md_read.rs::a_shell_index_file_means_the_section_is_absent_even_if_the_db_has_rows`
/// （那层才能同时握着库和文件两边）。
#[test]
fn an_empty_shell_index_file_produces_no_section() {
    assert!(from_md(Some("# 记忆索引\n\n"), &ages(&[]), NOW, LEAD_IN_PROJECT).is_none());
    let with_one = index_md(&[&pointer("a", "甲")]);
    assert!(
        from_md(Some(&with_one), &ages(&[]), NOW, LEAD_IN_PROJECT).is_some(),
        "对照：同一份夹具里加一行指针就该出块"
    );
}

#[test]
fn a_missing_or_empty_file_is_just_no_section() {
    assert!(from_md(None, &ages(&[]), NOW, LEAD_IN_PROJECT).is_none());
    assert!(
        from_md(Some(""), &ages(&[]), NOW, LEAD_IN_PROJECT).is_none(),
        "空文件同档"
    );
}

#[test]
fn pointer_extraction_itself_reports_none_when_the_body_has_no_pointer_line() {
    // 「空壳 md 不出块」在 `from_md` 之上有**两道**判空：`pointer_lines_from_md` 末尾的
    // `then_some` 与 `finish` 开头的 `is_empty`。**变异实测证明这不是纸面担心**：本批
    // 第一轮各摘其一（另一道留在场），`from_md` 的结果一字未变、两条判据双双跑绿
    // ⇒ 单摘任一道都抓不到。因此这里**直接钉私有函数自己的契约**（doc 明写
    // `None` = 一个指针行都没有），让第一道有独有承重点。
    // ⚠ 第二道的现状如实订正（刀②c-1）：rows 档退役之后 `finish` 只可能拿到非空 lines，
    // 那道 `is_empty` 已成**防御性冗余、没有独有证点**。留着它不是因为有人证过它承重，
    // 而是删它不会让任何测试变红、留它能让"哪天又来一个数据源"不至于注一个空节进来。
    // 这一格不许被读成"两道都有红证"。
    assert_eq!(pointer_lines_from_md("# 记忆索引\n\n"), None);
    assert_eq!(pointer_lines_from_md("<!-- 人写的批注 -->\n"), None);
    assert_eq!(
        pointer_lines_from_md("# t\n\n- [a](a.md) — x\n"),
        Some(vec!["- [a](a.md) — x".to_string()]),
        "有指针行时原样透传（不重排不改写）"
    );
}

#[test]
fn headings_blank_lines_and_html_comments_are_stripped() {
    // 人写的批注（HTML 注释）不许进上下文；跨行注释也要吞掉
    let md = "# 记忆索引\n\n<!-- 这是批注\n还有第二行\n-->\n\n\
              - [a](a.md) — 甲\n<!-- 单行批注 -->\n- [b](b.md) — 乙\n";
    let out = from_md(Some(md), &ages(&[]), NOW, LEAD_IN_PROJECT).expect("两条指针行");
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
    let out = from_md(Some(md), &ages(&[]), NOW, LEAD_IN_PROJECT).unwrap();
    assert!(!out.contains("随手写的一句话"), "非指针行不进: {out}");
    assert!(out.contains("- [a](a.md)"), "{out}");
}

// ---------------------------------------------------------------- 陈旧标注

/// 陈旧标注（稿 §4.1 那句 `（N days old）`）由注入侧按 `updated_at` **现算**并贴到
/// 行尾。为什么这一维不能由写侧落进文件：落盘时写死的那句"3 天前"在没人再写它的
/// 日子里**永远不会自己变陈旧**，而新鲜度只有 mtime / updated_at 一个信号（稿 §2 纪律 5）。
///
/// 这一维在刀②b-3 换读源时被静默丢过一次（当时只有 `from_rows` 那档在算它，md 档
/// 拿到的是纯文本行）⇒ 本条与下面那条"对不上号就不标"是补回来的红证。
#[test]
fn an_aged_pointer_line_is_marked_and_a_fresh_one_is_not() {
    let md = index_md(&[
        &pointer("user.lang", "中文"),
        &pointer("proj.rule", "别动 lockfile"),
        &pointer("no.tag", ""),
    ]);
    let table = ages(&[
        ("user.lang", NOW - 60),      // 1 分钟内 = 不标
        ("proj.rule", NOW - 3 * DAY), // >48h = 标
        ("no.tag", NOW - 10 * DAY),
    ]);
    let out = from_md(Some(&md), &table, NOW, LEAD_IN_PROJECT).unwrap();
    let lines = body_lines(&out);
    assert!(
        lines[0].ends_with("— 中文"),
        "负对照：没过 48h 的不许带标注：{}",
        lines[0]
    );
    assert!(
        lines[1].ends_with("（3 days old）"),
        "陈旧那条必须带上新鲜度：{}",
        lines[1]
    );
    assert!(lines[2].ends_with("（10 days old）"), "{}", lines[2]);
    assert!(
        lines.iter().all(|l| l.starts_with("- [")),
        "标注是**追加**在原行尾，不是把行换成别的形状：{lines:?}"
    );
}

/// `ages` 表里没有对应文件名时（人自己在索引里加的一行、或某条事实文件已删而索引
/// 还没刷）**不许硬贴标注、也不许把那一行丢掉**。前者是造假新鲜度，后者是让一行看得见
/// 的指针因为一个派生量缺席而消失。
#[test]
fn a_pointer_line_that_matches_no_fact_carries_no_marker() {
    let md = index_md(&[&pointer("hand.written", "人手加的一行")]);
    let out = from_md(Some(&md), &ages(&[]), NOW, LEAD_IN_PROJECT).unwrap();
    let lines = body_lines(&out);
    assert_eq!(lines.len(), 1, "对不上号也不许丢行：{lines:?}");
    assert_eq!(
        lines[0], "- [hand.written](hand.written.md) — 人手加的一行",
        "一个字都不许加：{lines:?}"
    );
}

// ---------------------------------------------------------------- 预算

/// 把 `n` 条指针行拼成一份必然超预算的 md（行形照 `refresh_index`）。
fn fat_index(n: usize) -> String {
    let lines: Vec<String> = (0..n).map(|i| pointer(&format!("k{i}"), "tag")).collect();
    index_md(&lines.iter().map(|s| s.as_str()).collect::<Vec<_>>())
}

#[test]
fn line_budget_truncates_with_a_pointing_note() {
    let out = from_md(
        Some(&fat_index(MAX_INDEX_LINES + 7)),
        &ages(&[]),
        NOW,
        LEAD_IN_PROJECT,
    )
    .unwrap();
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
    assert!(out.contains("[k0]("), "新→旧的头部要在: {out}");
    assert!(!out.contains("[k206]("), "超出的那条不该进来");
}

#[test]
fn char_budget_truncates_before_the_line_budget_when_lines_are_fat() {
    // **要靠长 key 撑行**才会撞字符预算：钩子（description）被 `one_line` 钳到 80 字符，
    // 第一版我拿 190 字符的 tags 当夹具，实际每行只有 ~90 字符、200 行 = 18k
    // 字符根本不撞线 ⇒ 那是一条空转判据（同族 = 台账「空转对照」）。
    // key 的合法长度上限是 512 字节，200 字符量级的 key 是真实落库形态。
    let wide = "x".repeat(200);
    let lines: Vec<String> = (0..MAX_INDEX_LINES)
        .map(|i| pointer(&format!("{wide}k{i}"), "t"))
        .collect();
    let md = index_md(&lines.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let out = from_md(Some(&md), &ages(&[]), NOW, LEAD_IN_PROJECT).unwrap();
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
    let out = from_md(
        Some(&fat_index(MAX_INDEX_LINES)),
        &ages(&[]),
        NOW,
        LEAD_IN_PROJECT,
    )
    .unwrap();
    assert!(!out.contains(TRUNCATED_NOTE), "刚好 200 条不算截断");
    assert_eq!(body_lines(&out).len(), MAX_INDEX_LINES);
}

// ------------------------------------------ 文件自带的截断标记（S1 刀②b-3）

/// 写侧已经把那句截断说明**写进文件**了，注入通路必须原样把它带给模型。
/// 少了这一半，形状就是"人打开文件夹看到『还有更多没进来』，而模型拿到的是一份
/// 看起来完整的索引"——比不钳更糟，因为它静默。
#[test]
fn a_truncated_index_file_keeps_the_warning_for_the_model() {
    let md = format!(
        "{}\n{TRUNCATED_NOTE}\n",
        index_md(&[&pointer("a", "甲"), &pointer("b", "乙")])
    );
    let out = from_md(Some(&md), &ages(&[]), NOW, LEAD_IN_PROJECT).expect("两条指针行");
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
    let md = fat_index(MAX_INDEX_LINES + 40);
    let out = from_md(Some(&md), &ages(&[]), NOW, LEAD_IN_PROJECT).expect("指针行充足");
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
