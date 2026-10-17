//! 直通与容错（自 `tests.rs` 批 F-16 拆出，用例本体逐字未改）：停用或零额度直通 /
//! 非 REQUEST 点 noop / 报文畸形当场拒（台账 T2 前这条叫 fail-open）/ 空工具名不计入会话热度。

use super::*;

use cordis_core::WaterfallListener;
use serde_json::json;
use session_store::Message;

#[tokio::test]
async fn disabled_or_zero_max_is_passthrough() {
    let tools = vec![spec("a", Exposure::OnDemand), spec("b", Exposure::OnDemand)];
    let l = listener(HashMap::new(), cfg_lock(false, 1));
    let (decision, out) = run(&l, tools.clone(), Vec::new()).await;
    assert!(matches!(decision, WaterfallDecision::Continue));
    assert_eq!(out.tools.len(), 2);
    assert!(!l.snapshot().enabled);

    let l2 = listener(HashMap::new(), cfg_lock(true, 0));
    let (d2, out2) = run(&l2, tools, Vec::new()).await;
    assert!(matches!(d2, WaterfallDecision::Continue));
    assert_eq!(out2.tools.len(), 2);
}

#[tokio::test]
async fn non_request_key_is_noop() {
    let l = listener(HashMap::new(), cfg_lock(true, 1));
    let payload = serde_json::to_value(RequestPayload {
        messages: Vec::new(),
        model: None,
        params: json!({}),
        tools: vec![spec("a", Exposure::OnDemand)],
        compacted: None,
        system: None,
        session_id: None,
        tree_directory: None,
    })
    .expect("payload");
    let mut v = payload.clone();
    let d = l.handle(points::PRE_STEP, &mut v).await;
    assert!(matches!(d, WaterfallDecision::Continue));
    assert_eq!(v, payload, "非 request 点：载荷一字未动（守卫根本没建立）");
}

/// 坏载荷 = 当场拒并点名（台账 T2）。
///
/// 原形状这条测的是 `Continue`（注释称 fail-open），但那份载荷出环照样在
/// `from_value(out.payload)?` 上失败——放行只是让后面几环各白跑一遍。
/// 现在 `by` 报出是哪个环先发现的；payload 被取空成 `Null` 是短路的副产物，
/// 出环的 Reject 分支不读它（`kernel/request.rs`）。
#[tokio::test]
async fn malformed_payload_rejects_and_names_this_ring() {
    let l = listener(HashMap::new(), cfg_lock(true, 1));
    let mut v = json!({ "messages": "garbage" });
    match l.handle(points::REQUEST, &mut v).await {
        WaterfallDecision::Reject(r) => assert_eq!(r.by, "ToolExposure"),
        other => panic!("expected reject, got {other:?}"),
    }
}

#[test]
fn session_counts_ignores_empty_tool_names() {
    let messages = vec![Message::ToolResult {
        call_id: plugin_api::CallId("x".into()),
        tool: String::new(),
        args: json!({}),
        output: json!(null),
        error: None,
    }];
    assert!(session_counts(&messages).is_empty());
}
