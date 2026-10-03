//! 自动标题纯函数与门禁测试（v2.57）：判定 / 题干 / 净化 / 旁车已有标题
//! 跳过。生成主链路（LLM 调用 + emit）不走单测——组件全是设置驱动的既有
//! 件，这里锁行为约定。

use super::{sanitize_title, should_generate, title_material, TITLE_MAX_CHARS};
use session_store::Message;

fn user_msg(s: &str) -> Message {
    Message::User {
        content: s.to_string(),
    }
}

#[test]
fn should_generate_gates_on_switch_sidecar_and_user_message() {
    let msgs = vec![user_msg("帮我看看这个 bug")];
    // 开 + 无旁车标题 + 有用户消息 = 生成
    assert!(should_generate(true, None, &msgs));
    // 手动重命名过（旁车有 title）永不覆盖
    assert!(!should_generate(true, Some("我起的名字"), &msgs));
    // 开关关
    assert!(!should_generate(false, None, &msgs));
    // 没有用户消息（异常轮）
    assert!(!should_generate(true, None, &[]));
}

#[test]
fn material_takes_first_user_and_first_nonempty_assistant() {
    let msgs = vec![
        user_msg("第一条"),
        Message::Assistant {
            content: String::new(),
            tool_calls: vec![],
        },
        Message::Assistant {
            content: "回复正文".into(),
            tool_calls: vec![],
        },
    ];
    let m = title_material(&msgs).unwrap();
    assert!(m.contains("用户：第一条"));
    assert!(m.contains("助手：回复正文"));

    // 没有用户消息 = None（上游就该跳过）
    assert!(title_material(&[Message::Assistant {
        content: "只有回复".into(),
        tool_calls: vec![],
    }])
    .is_none());
}

#[test]
fn sanitize_trims_quotes_newlines_and_length() {
    // 包裹引号剥掉
    assert_eq!(sanitize_title("「会话标题」").as_deref(), Some("会话标题"));
    assert_eq!(
        sanitize_title("  \"quoted title\" ").as_deref(),
        Some("quoted title")
    );
    // 多行只取首行
    assert_eq!(sanitize_title("第一行\n第二行").as_deref(), Some("第一行"));
    // 超长截 TITLE_MAX_CHARS
    let long = "长".repeat(TITLE_MAX_CHARS + 10);
    let got = sanitize_title(&long).unwrap();
    assert_eq!(got.chars().count(), TITLE_MAX_CHARS);
    // 空 = None
    assert!(sanitize_title("  \n ").is_none());
    assert!(sanitize_title("\"\"").is_none());
}
