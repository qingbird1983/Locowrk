//! `event_bus` 模块单测：on / emit / off 语义与顺序保证；
//! parallel / serial / waterfall 三种分发语义（§3.5）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::{
    EventBus, Listener, ListenerId, RejectReason, SerialListener, SerialOutcome, WaterfallDecision,
    WaterfallListener,
};

#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<String>>,
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl Listener for Recorder {
    async fn on_event(&self, key: &str, _payload: &Value) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.events.lock().unwrap().push(key.to_string());
    }
}

#[tokio::test]
async fn emit_notifies_registered_listeners_in_order() {
    let bus = EventBus::new();
    let a = Arc::new(Recorder::default());
    let b = Arc::new(Recorder::default());
    bus.on("agent/pre-step", Arc::clone(&a) as Arc<dyn Listener>);
    bus.on("agent/pre-step", Arc::clone(&b) as Arc<dyn Listener>);

    bus.emit("agent/pre-step", &json!({"step": 1})).await;

    assert_eq!(a.calls.load(Ordering::Relaxed), 1);
    assert_eq!(b.calls.load(Ordering::Relaxed), 1);
    // 同 key 监听器按注册顺序通知（确定性，无并发重排）
    assert_eq!(
        *a.events.lock().unwrap(),
        vec!["agent/pre-step".to_string()]
    );
}

#[tokio::test]
async fn emit_unknown_key_is_noop() {
    let bus = EventBus::new();
    // 不 panic、不等待：空监听器集合直接返回
    bus.emit("nobody/listens", &json!(null)).await;
}

#[tokio::test]
async fn off_removes_listener() {
    let bus = EventBus::new();
    let a = Arc::new(Recorder::default());
    let id: ListenerId = bus.on("agent/pre-step", Arc::clone(&a) as Arc<dyn Listener>);
    assert!(bus.off("agent/pre-step", id));
    assert!(!bus.off("agent/pre-step", id)); // 已移除，再 off 返回 false

    bus.emit("agent/pre-step", &json!(null)).await;
    assert_eq!(a.calls.load(Ordering::Relaxed), 0);
}

// ---- parallel（§3.5：并发通知所有监听者） ----

#[tokio::test]
async fn parallel_notifies_every_listener() {
    let bus = EventBus::new();
    let a = Arc::new(Recorder::default());
    let b = Arc::new(Recorder::default());
    bus.on("cache/flush", Arc::clone(&a) as Arc<dyn Listener>);
    bus.on("cache/flush", Arc::clone(&b) as Arc<dyn Listener>);

    bus.parallel("cache/flush", &json!({"dirty": true})).await;

    // join 全部任务后再返回：两个监听器都已完成（不 fire-and-forget）
    assert_eq!(a.calls.load(Ordering::Relaxed), 1);
    assert_eq!(b.calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn parallel_unknown_key_is_noop() {
    let bus = EventBus::new();
    bus.parallel("nobody/listens", &json!(null)).await;
}

// ---- serial（§3.5：顺序通知、可短路） ----

struct Gate {
    /// 返回的放行值序列；用尽后默认放行。
    verdicts: Mutex<Vec<bool>>,
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl SerialListener for Gate {
    async fn on_event(&self, _key: &str, _payload: &Value) -> bool {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.verdicts.lock().unwrap().pop().unwrap_or(true)
    }
}

#[tokio::test]
async fn serial_stops_at_first_veto() {
    let bus = EventBus::new();
    let a = Arc::new(Gate {
        verdicts: Mutex::new(vec![true]),
        calls: AtomicUsize::new(0),
    });
    let b = Arc::new(Gate {
        verdicts: Mutex::new(vec![false]),
        calls: AtomicUsize::new(0),
    });
    let c = Arc::new(Gate {
        verdicts: Mutex::new(vec![]),
        calls: AtomicUsize::new(0),
    });
    bus.on_serial("audit/chain", Arc::clone(&a) as Arc<dyn SerialListener>);
    bus.on_serial("audit/chain", Arc::clone(&b) as Arc<dyn SerialListener>);
    bus.on_serial("audit/chain", Arc::clone(&c) as Arc<dyn SerialListener>);

    let outcome = bus.serial("audit/chain", &json!(null)).await;

    assert_eq!(outcome, SerialOutcome::Stopped { at: 1 });
    assert_eq!(a.calls.load(Ordering::Relaxed), 1);
    assert_eq!(b.calls.load(Ordering::Relaxed), 1);
    assert_eq!(c.calls.load(Ordering::Relaxed), 0, "短路后不再通知");
}

#[tokio::test]
async fn serial_completes_when_all_allow() {
    let bus = EventBus::new();
    bus.on_serial(
        "audit/chain",
        Arc::new(Gate {
            verdicts: Mutex::new(vec![]),
            calls: AtomicUsize::new(0),
        }) as Arc<dyn SerialListener>,
    );
    assert_eq!(
        bus.serial("audit/chain", &json!(null)).await,
        SerialOutcome::Completed
    );
}

// ---- waterfall（§3.5：顺序经过、可改写/拒绝/重试） ----

struct Rewriter {
    /// 依次返回的决策；用尽后默认 Continue。
    decisions: Mutex<Vec<WaterfallDecision>>,
}

#[async_trait::async_trait]
impl WaterfallListener for Rewriter {
    async fn handle(&self, _key: &str, _payload: &mut Value) -> WaterfallDecision {
        self.decisions
            .lock()
            .unwrap()
            .pop()
            .unwrap_or(WaterfallDecision::Continue)
    }
}

fn rewriter(decisions: Vec<WaterfallDecision>) -> Arc<dyn WaterfallListener> {
    Arc::new(Rewriter {
        decisions: Mutex::new(decisions.into_iter().rev().collect()),
    })
}

#[tokio::test]
async fn waterfall_without_listeners_passes_through() {
    let bus = EventBus::new();
    let out = bus.waterfall("agent/request", json!({"v": 1})).await;
    assert_eq!(out.decision, WaterfallDecision::Continue);
    assert_eq!(out.payload, json!({"v": 1}));
}

#[tokio::test]
async fn waterfall_chains_rewrites_in_order() {
    let bus = EventBus::new();
    bus.on_waterfall(
        "agent/request",
        rewriter(vec![WaterfallDecision::Replace(json!({"v": 1}))]),
    );
    bus.on_waterfall(
        "agent/request",
        rewriter(vec![WaterfallDecision::Replace(json!({"v": 2}))]),
    );

    let out = bus.waterfall("agent/request", json!({"v": 0})).await;

    assert_eq!(out.decision, WaterfallDecision::Continue);
    assert_eq!(out.payload, json!({"v": 2}), "后一环改写覆盖前一环");
}

#[tokio::test]
async fn waterfall_reject_and_retry_short_circuit() {
    let bus = EventBus::new();
    bus.on_waterfall(
        "agent/request",
        rewriter(vec![WaterfallDecision::Reject(RejectReason {
            by: "approval-plugin".into(),
            reason: "policy violation".into(),
        })]),
    );
    bus.on_waterfall("agent/request", rewriter(vec![]));

    let out = bus.waterfall("agent/request", json!({"v": 0})).await;
    assert_eq!(
        out.decision,
        WaterfallDecision::Reject(RejectReason {
            by: "approval-plugin".into(),
            reason: "policy violation".into()
        })
    );

    let bus2 = EventBus::new();
    bus2.on_waterfall(
        "agent/request-error",
        rewriter(vec![WaterfallDecision::Retry {
            delay: Duration::from_millis(5),
        }]),
    );
    bus2.on_waterfall("agent/request-error", rewriter(vec![]));
    let out2 = bus2.waterfall("agent/request-error", json!(null)).await;
    assert_eq!(
        out2.decision,
        WaterfallDecision::Retry {
            delay: Duration::from_millis(5)
        }
    );
}

#[tokio::test]
async fn off_covers_serial_and_waterfall_registries() {
    let bus = EventBus::new();
    let id = bus.on_waterfall("agent/request", rewriter(vec![]));
    assert!(bus.off("agent/request", id));
    let out = bus.waterfall("agent/request", json!({"v": 1})).await;
    assert_eq!(
        out.decision,
        WaterfallDecision::Continue,
        "off 后监听器不再参与"
    );

    let id2 = bus.on_serial(
        "audit/chain",
        Arc::new(Gate {
            verdicts: Mutex::new(vec![]),
            calls: AtomicUsize::new(0),
        }),
    );
    assert!(bus.off("audit/chain", id2));
}
