//! subagent_verify 单测：转录紧凑化（裁剪/配对）、裁决解析（围栏/裸
//! JSON/解析失败降级）、磁盘往返。LLM 调用面不在此测（组装点走真实
//! AppLlm，走查实机覆盖）。

use std::path::PathBuf;

use plugin_api::{CallId, SideEffect};
use semver::Version;
use serde_json::{json, Value};
use session_store::{append_event, SessionEvent, TurnId, TurnOutcome};

use super::{build_transcript, parse_verdict};

fn sample_events() -> Vec<SessionEvent> {
    vec![
        SessionEvent::TurnStart {
            turn_id: TurnId(1),
            ts: 0,
        },
        SessionEvent::UserMessage {
            content: "重构 foo 模块并跑测试".into(),
            parts: Vec::new(),
        },
        SessionEvent::Thinking {
            text: "内部推理".into(),
        },
        SessionEvent::AssistantMessage {
            content: "好的，开始重构".into(),
            tool_calls: vec![CallId("c1".into())],
        },
        SessionEvent::ToolCall {
            call_id: CallId("c1".into()),
            tool: "shell_run".into(),
            args: json!({"command": "cargo test"}),
            plugin_id: None,
            plugin_version: Version::new(0, 1, 0),
            side_effect: SideEffect::Pure,
        },
        SessionEvent::ToolResult {
            call_id: CallId("c1".into()),
            output: json!("test result: ok. 12 passed"),
            error: None,
        },
        SessionEvent::AssistantMessage {
            content: "重构完成 ✓".into(),
            tool_calls: Vec::new(),
        },
        SessionEvent::TurnEnd {
            turn_id: TurnId(1),
            outcome: TurnOutcome::Completed,
            ts: 1,
        },
    ]
}

#[test]
fn transcript_keeps_evidence_skips_noise() {
    let t = build_transcript(&sample_events());
    assert!(t.contains("[任务] 重构 foo 模块并跑测试"));
    assert!(t.contains("[助手] 好的，开始重构"));
    assert!(t.contains("[调用 c1] shell_run"));
    assert!(t.contains("test result: ok"));
    // Thinking 与轮界事件不是完成度证据，不进转录
    assert!(!t.contains("内部推理"));
}

#[test]
fn transcript_clips_long_messages() {
    let long = "x".repeat(5000);
    let events = vec![SessionEvent::UserMessage {
        content: long.clone(),
        parts: Vec::new(),
    }];
    let t = build_transcript(&events);
    assert!(t.chars().count() < long.chars().count());
    assert!(t.trim_end().ends_with('…'));
}

/// ToolResult 错误面如实进转录（验收员要能看到失败证据）。
#[test]
fn transcript_marks_failed_results() {
    let events = vec![SessionEvent::ToolResult {
        call_id: CallId("c9".into()),
        output: Value::Null,
        error: Some("command failed".into()),
    }];
    let t = build_transcript(&events);
    assert!(t.contains("失败"));
    assert!(t.contains("command failed"));
}

#[test]
fn verdict_parses_bare_and_fenced_json() {
    let (status, criteria, _) = parse_verdict(
        "{\"status\":\"verified\",\"criteria\":[],\"summary\":\"ok\"}",
        &["测试通过".into()],
    );
    assert_eq!(status, "verified");
    assert_eq!(criteria, json!([]));

    let fenced = "说明如下\n```json\n{\"status\":\"failed\",\"criteria\":[{\"criterion\":\"测试通过\",\"verdict\":\"fail\",\"evidence\":\"测试失败\"}],\"summary\":\"没过\"}\n```";
    let (status, criteria, summary) = parse_verdict(fenced, &["测试通过".into()]);
    assert_eq!(status, "failed");
    assert_eq!(summary, "没过");
    assert_eq!(criteria[0]["verdict"], "fail");
}

#[test]
fn verdict_unparseable_falls_inconclusive() {
    let (status, criteria, _) = parse_verdict(
        "我觉得差不多完成了",
        &["测试通过".into(), "文件已改".into()],
    );
    assert_eq!(status, "inconclusive");
    assert_eq!(criteria.as_array().map(Vec::len), Some(2));
    assert_eq!(criteria[0]["verdict"], "inconclusive");
}

/// 端到端（不带 LLM）：生产同款 append_event 落盘 → load_session 读回 →
/// 转录含任务行。无 tempfile 依赖（scheduler/memory 测试同款）：pid 隔离
/// + 收尾删。
#[test]
fn transcript_roundtrips_from_disk() {
    let dir = std::env::temp_dir().join(format!("locowrk-verify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path: PathBuf = dir.join("p1-sub-1.jsonl");
    for e in sample_events() {
        append_event(&path, &e).expect("append");
    }
    let (session, skipped) = session_store::load_session(&path).expect("load");
    assert_eq!(skipped, 0);
    assert!(!session.events().is_empty());
    let t = build_transcript(session.events());
    assert!(t.contains("[任务] 重构 foo 模块并跑测试"));
    let _ = std::fs::remove_dir_all(&dir);
}
