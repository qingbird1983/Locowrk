//! `event_bus` 模块单测：on / emit / off 语义与顺序保证。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{EventBus, Listener, ListenerId};

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
