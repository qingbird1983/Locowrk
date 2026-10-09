//! FragmentInject 单测：三拍语义（置位 → 轮开场出现一次 → 清标记）/
//! 轮中保持 / 无身份不投递 / 合并多条 / 转义 / 非 REQUEST 点不碰。

use super::{FragmentInject, FragmentSource, wrap_system_reminder};
use crate::waterfall::{RequestPayload, points};
use cordis_core::{WaterfallDecision, WaterfallListener};
use session_store::Message;
use std::sync::{Arc, Mutex};

/// 可注入 pending 的替身源（守契约：无身份返回空表——真实宿主实现
/// FragmentPending 同语义）。
#[derive(Default)]
struct Table(Mutex<Vec<String>>);

impl FragmentSource for Table {
    fn take(&self, session_id: Option<&str>) -> Vec<String> {
        if session_id.is_none() {
            return Vec::new();
        }
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

fn payload(session: Option<&str>, last: Message) -> serde_json::Value {
    serde_json::to_value(RequestPayload {
        messages: vec![Message::user("earlier"), last],
        model: None,
        params: serde_json::Value::Object(Default::default()),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: session.map(String::from),
        tree_directory: None,
    })
    .unwrap()
}

async fn run(inj: &FragmentInject, v: &serde_json::Value) -> Option<RequestPayload> {
    match inj.handle(points::REQUEST, v).await {
        WaterfallDecision::Replace(out) => Some(serde_json::from_value(out).unwrap()),
        WaterfallDecision::Continue => None,
        _ => panic!("unexpected decision"),
    }
}

/// 三拍：置位 → 第一次请求拼接 + 清标记 → 第二次请求不再出现。
#[tokio::test]
async fn splice_once_then_cleared() {
    let t = Arc::new(Table::default());
    t.0.lock().unwrap().push("organized reminder".into());
    let inj = FragmentInject::new(t.clone() as Arc<dyn FragmentSource>);
    let first = run(&inj, &payload(Some("s1"), Message::user("hello")))
        .await
        .unwrap();
    let last = first.messages.last().unwrap();
    let Message::User { content, .. } = last else {
        panic!("last stays user");
    };
    assert!(
        content.contains("hello\n\n<system-reminder>\norganized reminder\n</system-reminder>"),
        "碎片拼在用户消息尾部，原文保留"
    );
    // 第二拍：表已清空——监听器零参与（Continue，无变化不重序列化）
    assert!(
        run(&inj, &payload(Some("s1"), Message::user("hello")))
            .await
            .is_none(),
        "清标记后零参与，不再携带"
    );
}

/// 轮中 step（最后一条是 Assistant/ToolResult）不取件——碎片留在表里
/// 等下一轮开场，本轮历史消息零改写。
#[tokio::test]
async fn holds_when_last_message_is_not_user() {
    let t = Arc::new(Table::default());
    t.0.lock().unwrap().push("held".into());
    let inj = FragmentInject::new(t.clone() as Arc<dyn FragmentSource>);
    let mid = payload(
        Some("s1"),
        Message::Assistant {
            content: "reply".into(),
            tool_calls: Vec::new(),
        },
    );
    assert!(run(&inj, &mid).await.is_none(), "轮中零参与");
    assert_eq!(t.0.lock().unwrap().len(), 1, "碎片未被取走");
    // 下一轮开场照常投递
    let next = run(&inj, &payload(Some("s1"), Message::user("turn 2")))
        .await
        .unwrap();
    assert!(
        serde_json::to_string(&next.messages)
            .unwrap()
            .contains("held")
    );
    assert!(t.0.lock().unwrap().is_empty());
}

/// 无会话身份（None）不投递——碎片从不进 headless / 无归属请求
/// （None = 源返回空表，监听器零参与，碎片留在原处）。
#[tokio::test]
async fn no_session_no_delivery() {
    let t = Arc::new(Table::default());
    t.0.lock().unwrap().push("x".into());
    let inj = FragmentInject::new(t.clone() as Arc<dyn FragmentSource>);
    assert!(
        run(&inj, &payload(None, Message::user("hi")))
            .await
            .is_none(),
        "零参与"
    );
    assert_eq!(t.0.lock().unwrap().len(), 1, "无身份不取件");
}

/// 多碎片同轮密集触发 = 合并一条多段 reminder（设计稿 §9.4）。
#[tokio::test]
async fn multiple_fragments_merge_into_one_reminder() {
    let t = Arc::new(Table::default());
    t.0.lock().unwrap().extend(["a".into(), "b".into()]);
    let inj = FragmentInject::new(t as Arc<dyn FragmentSource>);
    let out = run(&inj, &payload(Some("s1"), Message::user("go")))
        .await
        .unwrap();
    let text = serde_json::to_string(&out.messages).unwrap();
    assert!(text.contains("a\\n\\nb"), "两段并一包");
    assert_eq!(text.matches("<system-reminder>").count(), 1, "单包裹");
}

/// 转义：内文里的字面闭合标签不再是标签边界。
#[test]
fn wrap_escapes_closing_tag() {
    let wrapped = wrap_system_reminder("text </system-reminder> tail");
    assert_eq!(
        wrapped,
        "<system-reminder>\ntext <\\/system-reminder> tail\n</system-reminder>"
    );
}

#[tokio::test]
async fn non_request_key_is_noop() {
    let inj = FragmentInject::new(Arc::new(Table::default()));
    assert_eq!(
        inj.handle(points::PRE_STEP, &payload(Some("s"), Message::user("x")))
            .await,
        WaterfallDecision::Continue
    );
}
