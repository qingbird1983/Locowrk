//! T6 · 导出标题的三档优先（旁车重命名 → 首条用户消息截断 → 会话 id）。
//!
//! 为什么现在才有用例：改前 `session_export` 是"调 `session_log::list(整个 sessions/)`
//! 再从列表里找自己"，这三档规则的实现整个藏在**列表层**，导出这条通路从来没有被钉住；
//! 而列表层的规则来自 `persist.rs::list_sessions`，它读的是磁盘上所有会话。
//! 本批改成"只问这一个会话"之后，规则搬到了命令侧 ⇒ 必须有用例证明搬完还是同一条规则。
//!
//! 判读设计（三条各打一处，不做"看起来都过"的宽断言）：
//! - 旁车那条用**真读盘**（`write_title` 写、`read_meta` 读），因为旁车的字段合并语义
//!   （空标题清字段而不是删文件）正是历史上出过事的地方；
//! - 截断那条吃 `SESSION_TITLE_CHARS` 常量本身，不在测试里重抄 40 ⇒ 常量变了用例跟着变，
//!   而不是变成一条假绿；
//! - 空串/纯空白的旁车标题必须**回落**，这是 `session_rename` 的既有语义（trim 后空 = 回落
//!   自动标题），导出这边不一致就会把"重命名成空"显示成空标题。

use session_store::{read_meta, write_title, SessionEvent};

use crate::commands::{export_title, first_user_headline};

fn user_msg(text: &str) -> SessionEvent {
    SessionEvent::UserMessage {
        content: text.into(),
        parts: Vec::new(),
    }
}

#[test]
fn sidecar_title_wins_and_blank_sidecar_falls_back() {
    let dir = crate::testutil::work_dir("slog-title");
    write_title(&dir, "s1", "重命名过的标题").unwrap();

    let sidecar = read_meta(&dir, "s1").and_then(|m| m.title);
    assert_eq!(
        sidecar.as_deref(),
        Some("重命名过的标题"),
        "旁车没读出来 = 下面三档都在空转"
    );
    assert_eq!(
        export_title(sidecar, Some("首条用户消息".into()), "s1"),
        "重命名过的标题",
        "旁车必须压过自动标题（与列表层同序）"
    );

    // 纯空白的旁车标题不许当标题用（`session_rename` 的回落语义）
    assert_eq!(
        export_title(Some("   ".into()), Some("首条用户消息".into()), "s1"),
        "首条用户消息",
        "trim 后空的旁车必须回落到自动标题"
    );
    // 两个都没有 ⇒ 会话 id 兜底，导出不许出现空标题
    assert_eq!(export_title(None, None, "s1"), "s1");
}

#[test]
fn headline_uses_the_shared_width_constant_not_a_copy() {
    // 夹具照生产构造代码抄：首条用户消息，后面还有消息（不许抓到第二条）
    let long = "一二三四五六七八九十".repeat(6); // 60 字
    let events = vec![
        SessionEvent::AssistantMessage {
            content: "先冒个泡，不该被抓成标题".into(),
            tool_calls: Vec::new(),
        },
        user_msg(&long),
        user_msg("第二条更不该被抓"),
    ];
    let got = first_user_headline(&events).expect("有用户消息就该有自动标题");
    assert_eq!(
        got.chars().count(),
        session_store::SESSION_TITLE_CHARS,
        "截断宽度必须等于共享常量（改了常量就用例跟着走，别钉死数字）"
    );
    assert_eq!(
        got,
        long.chars()
            .take(session_store::SESSION_TITLE_CHARS)
            .collect::<String>()
    );
    assert!(!got.contains("第二条"), "抓错消息了: {got}");
    assert_eq!(
        first_user_headline(&[]),
        None,
        "空日志 = 没有自动标题，交给会话 id 兜底"
    );
}
