//! 断裂诊断单测：纯追加健康 / system·tools·逐消息断裂定位 / 截断报告 /
//! 无会话身份跳过。

use kernel_agent::LlmRequest;
use plugin_api::ToolSpec;
use session_store::Message;

use super::Diagnoser;

fn req(system: &str, msgs: &[&str], session: &str) -> LlmRequest {
    LlmRequest {
        messages: msgs
            .iter()
            .map(|m| Message::User {
                content: (*m).to_string(),
                parts: Vec::new(),
            })
            .collect(),
        tools: vec![ToolSpec {
            name: "t1".into(),
            description: "d".into(),
            schema: serde_json::json!({"type": "object"}),
            side_effect: plugin_api::SideEffect::Pure,
            exposure: plugin_api::Exposure::Always,
        }],
        model: None,
        params: serde_json::Value::Null,
        system: Some(system.to_string()),
        session_id: Some(session.to_string()),
    }
}

/// S4 刀③c：system 不再有记忆索引段（记忆线整体退役），夹具回到生产实际形态
/// ——静态身份层 + 思考语言两句，无协议标记。
const SYS_V1: &str = "base prefix\nthink in the user's language";
const SYS_V2_CHANGED: &str = "base prefix\nthink in the user's language (v2)";

#[test]
fn pure_append_is_healthy() {
    let d = Diagnoser::default();
    let r1 = d.record(&req(SYS_V1, &["hi"], "s1")).unwrap();
    assert!(r1.healthy(), "首轮无基线 = 健康: {:?}", r1.breaks);
    let r2 = d.record(&req(SYS_V1, &["hi", "追加轮"], "s1")).unwrap();
    assert!(r2.healthy());
    assert_eq!(r2.step, 2);
    assert_eq!(r2.prev_messages, 1);
}

#[test]
fn system_change_is_reported_as_base() {
    let d = Diagnoser::default();
    let _ = d.record(&req(SYS_V1, &["hi"], "s2"));
    let r = d.record(&req(SYS_V2_CHANGED, &["hi", "x"], "s2")).unwrap();
    assert_eq!(r.breaks, vec!["base"], "无标记段 = 整块 base 变化");
    let unchanged = d
        .record(&req(SYS_V2_CHANGED, &["hi", "x", "y"], "s2"))
        .unwrap();
    assert!(unchanged.healthy(), "system 没动不误报");
}

#[test]
fn tool_set_change_is_reported() {
    let d = Diagnoser::default();
    let mut r1 = req(SYS_V1, &["hi"], "s3");
    let _ = d.record(&r1);
    r1.tools[0].description = "changed".into();
    let r = d.record(&r1).unwrap();
    assert!(r.breaks.contains(&"tools".to_string()), "{:?}", r.breaks);
}

#[test]
fn message_rewrite_reports_first_break_index() {
    let d = Diagnoser::default();
    let _ = d.record(&req(SYS_V1, &["a", "b", "c"], "s4"));
    let r = d.record(&req(SYS_V1, &["a", "B!", "c"], "s4")).unwrap();
    assert_eq!(r.breaks, vec!["message#1"], "首个不一致位即断点");
}

#[test]
fn truncation_is_flagged() {
    let d = Diagnoser::default();
    let _ = d.record(&req(SYS_V1, &["a", "b", "c"], "s5"));
    let r = d.record(&req(SYS_V1, &["a"], "s5")).unwrap();
    assert!(r.breaks.contains(&"messages-truncated".to_string()));
}

#[test]
fn anonymous_requests_are_skipped() {
    let d = Diagnoser::default();
    let mut r = req(SYS_V1, &["hi"], "");
    r.session_id = None;
    assert!(d.record(&r).is_none());
    assert!(
        d.record(&req(SYS_V1, &["hi"], "")).is_none(),
        "空串身份同跳过"
    );
}
