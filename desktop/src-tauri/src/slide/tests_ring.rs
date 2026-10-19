//! 刀⑤ 接线面（环本体）：`slide/tests.rs` 那九条全绿也只证明 `plan` 自己对，
//! 这里把注册后的**行为**跑到终态。夹具（`rounds` / `over_line`）住在被测模块
//! `slide.rs` 里，两个测试文件共用（同 `compact.rs` 的 `sample_summary` 那条理由）。

use kernel_agent::RequestPayload;
use serde_json::json;

use super::*;
use crate::compact::MAX_COMPACT_FAILURES;

// ---------- 接线面（环本体）：上面那些全绿也只证明 `plan` 自己对 ----------
//
// 三条各钉一种"单元测了但生产里不发生"的漂法：
// ① 环没有真的改写**发出去的载荷**（守卫还回去的是改过的包？）；
// ② 熔断表读了另一张（`Failures` 与主层不是同一份 ⇒ 界面上关了这层它还开着）；
// ③ 压缩总开关关不掉这一层。

fn settings_with(context: Value) -> Arc<SettingsStore> {
    let root = crate::testutil::work_dir("slide-ring");
    std::fs::write(
        root.join("settings.json"),
        json!({"context": context}).to_string(),
    )
    .unwrap();
    Arc::new(SettingsStore::open(&root).unwrap())
}

fn bus_with(
    settings: Arc<SettingsStore>,
    failures: Arc<Failures>,
    main: Option<u64>,
) -> cordis_core::EventBus {
    let bus = cordis_core::EventBus::new();
    bus.on_waterfall(
        points::REQUEST,
        Arc::new(Slide::new(settings, failures, Arc::new(move |_| main))),
    );
    bus
}

/// 熔断到线：把 `Failures` 记满三次，而不是给判据塞一个 `true`。
/// 这样这条用例同时证明"表 → fused → 这一层醒"整条链是通的（判据 SS22a 的行为面）。
fn fused_failures(sid: &str) -> Arc<Failures> {
    let f = Arc::new(Failures::default());
    for _ in 0..MAX_COMPACT_FAILURES {
        f.record(sid);
    }
    f
}

fn payload_of(messages: Vec<Message>, session: Option<&str>) -> Value {
    serde_json::to_value(RequestPayload {
        messages,
        model: None,
        params: json!({}),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: session.map(|s| s.to_string()),
        tree_directory: None,
    })
    .unwrap()
}

fn req_of(v: Value) -> RequestPayload {
    serde_json::from_value(v).unwrap()
}

#[tokio::test]
async fn the_ring_shortens_the_payload_it_sends() {
    let msgs = rounds(7, 4_000);
    let settings = settings_with(json!({"thresholdTokens": 100_000}));
    let bus = bus_with(
        Arc::clone(&settings),
        fused_failures("s-slide"),
        Some(estimate_tokens(&msgs)),
    );
    let out = bus
        .waterfall(points::REQUEST, payload_of(msgs.clone(), Some("s-slide")))
        .await;
    assert!(
        matches!(out.decision, WaterfallDecision::Continue),
        "这一环不许短路决策环，实测 {:?}",
        out.decision
    );
    let req = req_of(out.payload);
    assert!(
        req.messages.len() < msgs.len(),
        "环没改到发出去的载荷：{} → {}",
        msgs.len(),
        req.messages.len()
    );
    // 提问一条不少：七轮 ⇒ 七条 User 全在。
    let q = req
        .messages
        .iter()
        .filter(|m| matches!(m, Message::User { .. }))
        .count();
    assert_eq!(q, 7, "环上这一趟把提问也丢了");
    // 过程只剩最近一段：一条 assistant + 一条回执。
    let kept_process = req
        .messages
        .iter()
        .filter(|m| !matches!(m, Message::User { .. }))
        .count();
    assert_eq!(kept_process, 2, "旧过程没被丢掉，这一层等于没跑");
    assert!(
        req.compacted.is_none(),
        "这一层不落账（理由见 slide.rs 头注）"
    );
}

/// 没有会话身份 ⇒ 没有熔断记录可读 ⇒ 当作没熔断。这一条钉的是"没依据时不许动手"：
/// 保护网唯一的依据就是那条会话的连败数，读不到就宁可让请求失败。
#[tokio::test]
async fn a_session_without_an_id_cannot_be_fused() {
    let msgs = rounds(6, 4_000);
    let settings = settings_with(json!({"thresholdTokens": 100_000}));
    let bus = bus_with(
        Arc::clone(&settings),
        fused_failures("s-other"),
        Some(estimate_tokens(&msgs)),
    );
    let out = bus
        .waterfall(points::REQUEST, payload_of(msgs.clone(), None))
        .await;
    let req = req_of(out.payload);
    assert_eq!(
        serde_json::to_string(&req.messages).unwrap(),
        serde_json::to_string(&msgs).unwrap(),
        "读不到熔断态却动了手 = 在没有依据的情况下丢过程"
    );
}

/// 压缩总开关关掉 = 整条压缩面不参与，包括这一层（AC-S2-10 那句"关 = 完全不压、
/// 接近窗口只提醒"把保护网一起算在内）。
#[tokio::test]
async fn the_master_switch_off_leaves_the_request_untouched() {
    let msgs = rounds(6, 4_000);
    let settings = settings_with(json!({"compactEnabled": false}));
    let bus = bus_with(
        Arc::clone(&settings),
        fused_failures("s-slide"),
        Some(estimate_tokens(&msgs)),
    );
    let out = bus
        .waterfall(points::REQUEST, payload_of(msgs.clone(), Some("s-slide")))
        .await;
    let req = req_of(out.payload);
    assert_eq!(
        serde_json::to_string(&req.messages).unwrap(),
        serde_json::to_string(&msgs).unwrap(),
        "开关关得掉才算可配置：这一层照旧动了手"
    );
}
