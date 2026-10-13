//! 摘要补写解析单测（队列 Q8c · CB §7.4 的「泄压留下的机械摘要谁来还」）：
//! `parse_summaries` 的取已知 id + 硬钳复用记录员那把尺 / 坏条目逐条跳过而不废整批 /
//! 允许少给 / 结构面仍严格，另加补写提示词「暂不分家族」。
//!
//! 名字出处 = `use super::*;` 经宿主 `manager/tests.rs` 的 glob 向上传导到
//! `manager`（形状闸 S6 那一档口径：簇用到的名字必须有出处，本簇不重复 import）。

use super::*;

fn candidate_ids() -> Vec<String> {
    ["b1", "b2", "b3"].iter().map(|s| s.to_string()).collect()
}

#[test]
fn parse_summaries_takes_known_ids_and_clamps_the_text() {
    let raw = r#"```json
[{"id":" b2 ","summary":"  改完了，测试也绿了。  "},{"id":"b1","summary":"查问题。"}]
```"#;
    let out = parse_summaries(raw, &candidate_ids()).unwrap();
    // 顺序照模型给的顺序（落笔按 id 定位，与顺序无关）；id 与正文都 trim
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].chapter_id, "b2");
    assert_eq!(out[0].summary, "改完了，测试也绿了。");
    assert_eq!(out[1].chapter_id, "b1");

    let long = format!(
        r#"[{{"id":"b1","summary":"{}"}}]"#,
        "字".repeat(SUMMARY_MAX_CHARS + 500)
    );
    let clamped = parse_summaries(&long, &candidate_ids()).unwrap();
    assert_eq!(
        clamped[0].summary.chars().count(),
        SUMMARY_MAX_CHARS,
        "硬钳复用记录员那把尺"
    );
}

/// 三种坏条目**逐条跳过**而不废整批（补写是增益操作），且跳过的三种各有断言：
/// 未知 id（模型自己编号）、空摘要（它按提示词省略却留了空串）、重复 id（取第一条）。
#[test]
fn parse_summaries_skips_bad_entries_instead_of_rejecting_the_batch() {
    let raw = r#"[{"id":"b9","summary":"编号是编的"},{"id":"b2","summary":"  "},
                  {"id":"b1","summary":"第一条"},{"id":"b1","summary":"同章第二条"},
                  {"id":"","summary":"没编号"}]"#;
    let out = parse_summaries(raw, &candidate_ids()).unwrap();
    assert_eq!(out.len(), 1, "五种形状各归一处，只有能对上的那条落笔");
    assert_eq!(
        (out[0].chapter_id.as_str(), out[0].summary.as_str()),
        ("b1", "第一条")
    );
}

/// **允许少给**（与 `parse_drafts` 的"必须铺满"相对）：只补了两章也照常返回；
/// 一章都对不上就得到空表——调用方把空表按失败记账，于是这条通路最多烧到
/// 上限次数自己收手，不会每轮重烧。
#[test]
fn parse_summaries_tolerates_missing_entries_and_returns_empty_on_all_unknown() {
    let part = r#"[{"id":"b3","summary":"第三段。"}]"#;
    let out = parse_summaries(part, &candidate_ids()).unwrap();
    assert_eq!(out.len(), 1);
    assert!(
        parse_summaries(r#"[{"id":"bX","summary":"都对不上"}]"#, &candidate_ids())
            .unwrap()
            .is_empty()
    );
    assert!(parse_summaries("[]", &candidate_ids()).unwrap().is_empty());
}

/// 结构面仍然严格：没有数组 = NoJson、数组形状不对 = BadEntry（与 `parse_drafts`
/// 同一套错误面，调用方的日志/降级分流不用为补写另判一次）。
#[test]
fn parse_summaries_reports_structural_failures_like_drafts() {
    assert_eq!(
        parse_summaries("好的，我没法补。", &candidate_ids()).unwrap_err(),
        MemoryManagerError::NoJson
    );
    assert!(matches!(
        parse_summaries(
            r#"[{"chapter_id":"b1","text":"旧字段名"}]"#,
            &candidate_ids()
        ),
        Err(MemoryManagerError::BadEntry(_))
    ));
}

/// 补写提示词当前只有通用版：四个家族取值必须逐字节相同（将来配方言稿时，
/// 这条用例会红着提醒"要么补三份 md，要么承认它不分家族"，而不是默默漂移）。
#[test]
fn summarizer_prompt_is_family_agnostic_until_dialects_are_measured() {
    let base = summarizer_prompt("default");
    for family in ["claude", "deepseek", "qwen", "nonsense", ""] {
        assert_eq!(summarizer_prompt(family), base, "{family} 家族回落通用版");
    }
    assert!(
        !base.contains("===SUMMARY==="),
        "补写稿不许借用记录员的段标记"
    );
    assert!(base.contains("只输出一个 JSON 数组"));
}
