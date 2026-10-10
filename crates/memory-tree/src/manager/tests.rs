//! recorder 单测：输出解析校验（铺满/衔接/净化）+ 规则切分（User 边界
//! 不断工具对）+ 提示词家族匹配。

use super::*;
use crate::material::{MaterialKind, build_items, transcript};
use session_store::SessionEvent;

fn items() -> Vec<MaterialItem> {
    let events = vec![
        SessionEvent::UserMessage {
            content: "第一轮：查问题".into(),
            parts: Vec::new(),
        },
        SessionEvent::AssistantMessage {
            content: "好的，先看配置".into(),
            tool_calls: vec![],
        },
        SessionEvent::UserMessage {
            content: "第二轮：改代码".into(),
            parts: Vec::new(),
        },
        SessionEvent::AssistantMessage {
            content: "改完了".into(),
            tool_calls: vec![],
        },
        SessionEvent::UserMessage {
            content: "第三轮：跑测试".into(),
            parts: Vec::new(),
        },
        SessionEvent::AssistantMessage {
            content: "测试绿了".into(),
            tool_calls: vec![],
        },
    ];
    build_items(&events, 0)
}

#[test]
fn parse_accepts_tiled_output_and_maps_to_event_positions() {
    let raw = r#"前言说明
```json
[
  {"title": "查问题", "summary": "第一轮讨论", "event_range": [0, 1]},
  {"title": "改代码", "summary": "第二轮讨论", "event_range": [2, 3]},
  {"title": "跑测试", "summary": "第三轮讨论", "event_range": [4, 5]}
]
```"#;
    let drafts = parse_drafts(raw, &items()).expect("合法铺满输出");
    assert_eq!(drafts.len(), 3);
    assert_eq!(drafts[0].event_range, (0, 1)); // 项下标 0–1 = 事件位 0–1
    assert_eq!(drafts[2].event_range, (4, 5));
}

#[test]
fn parse_rejects_gaps_overlap_and_out_of_range() {
    let good = r#"[{"title":"a","summary":"s","event_range":[0,2]},{"title":"b","summary":"s","event_range":[3,5]}]"#;
    assert!(parse_drafts(good, &items()).is_ok());

    let gap = r#"[{"title":"a","summary":"s","event_range":[0,1]},{"title":"b","summary":"s","event_range":[3,5]}]"#;
    assert!(matches!(
        parse_drafts(gap, &items()),
        Err(MemoryManagerError::Coverage(_))
    ));

    let short = r#"[{"title":"a","summary":"s","event_range":[0,3]}]"#;
    assert!(matches!(
        parse_drafts(short, &items()),
        Err(MemoryManagerError::Coverage(_))
    ));

    let overlap = r#"[{"title":"a","summary":"s","event_range":[0,3]},{"title":"b","summary":"s","event_range":[2,5]}]"#;
    assert!(matches!(
        parse_drafts(overlap, &items()),
        Err(MemoryManagerError::Coverage(_))
    ));
}

#[test]
fn parse_rejects_garbage_and_empty_entries() {
    assert_eq!(
        parse_drafts("我觉得挺好看的", &items()),
        Err(MemoryManagerError::NoJson)
    );
    let empty_title = r#"[{"title":"  ","summary":"s","event_range":[0,5]}]"#;
    assert!(matches!(
        parse_drafts(empty_title, &items()),
        Err(MemoryManagerError::BadEntry(_))
    ));
    assert!(parse_drafts("[]", &items()).is_err());
}

#[test]
fn rule_split_respects_user_boundaries_and_fills_all() {
    // chunk 极小：每条 User 边界断章节；结果必须铺满全部材料项
    let drafts = rule_split_drafts(&items(), 1);
    assert_eq!(drafts.len(), 3);
    assert_eq!(drafts[0].event_range, (0, 1));
    assert_eq!(drafts[1].event_range, (2, 3));
    assert_eq!(drafts[2].event_range, (4, 5));
    assert!(drafts[0].title.contains("第一轮"));
    assert!(drafts[0].summary.starts_with("（规则切分）"));
}

#[test]
fn rule_split_big_chunk_makes_single_bundle() {
    let drafts = rule_split_drafts(&items(), 1_000_000);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].event_range, (0, 5));
}

#[test]
fn rule_split_never_splits_tool_pairing() {
    // 用户消息后跟一长串工具往返（无 User 边界）：预算超时也只能等到
    // 下一处 User 才断——章节内不出现半截工具对
    let events = vec![
        SessionEvent::UserMessage {
            content: "开头".into(),
            parts: Vec::new(),
        },
        SessionEvent::AssistantMessage {
            content: String::new(),
            tool_calls: vec![],
        },
        SessionEvent::ToolCall {
            call_id: plugin_api::CallId("c".into()),
            tool: "fs_read".into(),
            args: serde_json::json!({"p": "x"}),
            plugin_id: None,
            plugin_version: semver::Version::new(0, 1, 0),
            side_effect: plugin_api::SideEffect::Pure,
        },
        SessionEvent::ToolResult {
            call_id: plugin_api::CallId("c".into()),
            output: serde_json::json!({"d": "y"}),
            error: None,
        },
        SessionEvent::UserMessage {
            content: "结尾".into(),
            parts: Vec::new(),
        },
    ];
    let its = build_items(&events, 0);
    let drafts = rule_split_drafts(&its, 1);
    assert_eq!(drafts.len(), 2);
    // 第一章节必须覆盖到 User(下标3) 之前——工具对（下标 1–2）完整在内
    assert_eq!(drafts[0].event_range, (its[0].event, its[2].event));
    assert_eq!(drafts[1].event_range, (its[3].event, its[3].event));
    assert_eq!(its[3].kind, MaterialKind::User);
}

#[test]
fn prompt_family_matching_and_verbatim_clause_present() {
    assert_eq!(family_of("claude-sonnet-4"), "claude");
    assert_eq!(family_of("DeepSeek-V4"), "deepseek");
    assert_eq!(family_of("qwen3-max"), "qwen");
    assert_eq!(family_of("gpt-x"), "default");
    // §5.6 逐字保留纪律在所有记忆管理器提示词里
    for f in ["default", "claude", "deepseek", "qwen"] {
        assert!(
            manager_prompt(f).contains("逐字保留"),
            "recorder/{f}.md 缺逐字保留纪律"
        );
        assert!(!main_prompt(f).is_empty());
    }
}

#[test]
fn user_parts_fold_to_placeholder_in_material() {
    // 批 E §10.2 挂捆边界降级（最小形态）：图片字节不进材料，占位符
    // 「[图片: name]」在场——记录员摘要还能提到它，捆边界后信息不归零
    let events = vec![
        SessionEvent::UserMessage {
            content: "看这张图".into(),
            parts: vec![session_store::ContentPart::Image {
                asset: session_store::AssetRef {
                    id: "abc".into(),
                    mime: "image/png".into(),
                    name: "shot.png".into(),
                },
            }],
        },
        SessionEvent::AssistantMessage {
            content: "好的".into(),
            tool_calls: vec![],
        },
    ];
    let its = build_items(&events, 0);
    assert_eq!(its.len(), 2);
    assert_eq!(its[0].kind, MaterialKind::User);
    assert!(
        its[0].text.contains("[图片: shot.png]"),
        "text = {}",
        its[0].text
    );
    assert!(its[0].text.contains("看这张图"));
    // 纯文本消息无占位（零噪声）
    let plain = vec![SessionEvent::UserMessage {
        content: "纯文字".into(),
        parts: Vec::new(),
    }];
    let its = build_items(&plain, 0);
    assert_eq!(its[0].text, "纯文字");
}

// ---- 轮级总结（Organize Summary Design v1 §3.4）----

#[test]
fn parse_drafts_ignores_trailing_summary_section() {
    // 总结正文带 [ ] 也不污染数组首尾定位（先切标记再取数组）
    let raw = concat!(
        "前言\n```json\n[",
        "{\"title\": \"a\", \"summary\": \"s\", \"event_range\": [0, 1]}, ",
        "{\"title\": \"b\", \"summary\": \"s\", \"event_range\": [2, 3]}, ",
        "{\"title\": \"c\", \"summary\": \"s\", \"event_range\": [4, 5]}]",
        "\n```\n===SUMMARY===\n本轮讨论了 [1M] 后缀与 [N] 行号，结论：都保留。"
    );
    let drafts = parse_drafts(raw, &items()).expect("数组部分照常解析");
    assert_eq!(drafts.len(), 3);
    let sum = run_summary(raw).expect("总结段可提取");
    assert!(sum.contains("都保留"));
    assert!(!sum.contains("===SUMMARY==="));
}

#[test]
fn run_summary_cases() {
    // 无标记 = None；标记后空白 = None
    let bare = r#"[{"title":"a","summary":"s","event_range":[0,0]}]"#;
    assert!(run_summary(bare).is_none());
    assert!(run_summary("[]\n===SUMMARY===\n   \n").is_none());
    // 空行折叠 + 前后 trim
    let s = run_summary("垃圾\n===SUMMARY===\n第一段。\n\n\n第二段。\n").unwrap();
    assert_eq!(s, "第一段。\n\n第二段。"); // 连续空行压到一段
    // 硬钳
    let long = format!(
        "垃圾===SUMMARY==={}",
        "长".repeat(RUN_SUMMARY_MAX_CHARS + 100)
    );
    let s = run_summary(&long).unwrap();
    assert_eq!(s.chars().count(), RUN_SUMMARY_MAX_CHARS);
}

#[test]
fn material_flags_run_summary_and_keeps_verbatim() {
    use crate::material::MaterialKind;
    let events = vec![
        SessionEvent::UserMessage {
            content: "早问".into(),
            parts: vec![],
        },
        SessionEvent::AssistantMessage {
            content: "早答".into(),
            tool_calls: vec![],
        },
        SessionEvent::ContextSummary {
            text: "上一段总结。".into(),
        },
        SessionEvent::UserMessage {
            content: "新问".into(),
            parts: vec![],
        },
    ];
    let its = build_items(&events, 0);
    assert_eq!(its.len(), 4);
    assert_eq!(its[2].kind, MaterialKind::Summary);
    assert_eq!(its[2].event, 2);
    // 总结逐字进转录（不截断），标签「总结」
    let t = transcript(&its);
    assert!(t.contains("[2] 总结: 上一段总结。"));
    // 非 User 不作规则切分切点（总结夹在中间不产生断口）
    let drafts = rule_split_drafts(&its, 1);
    assert!(!drafts.is_empty());
}
