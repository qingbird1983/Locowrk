//! memory_inject 单测：注入格式 / 配置开关 / 空表降级 / 超长截断 /
//! 追加语义 / 非 REQUEST 点不碰。
//! 批 F-16 目录化：共享假件与跑批入口留本文件，用例按面分簇见 `render` /
//! `prefetch` / `scope_context`。

mod prefetch;
mod render;
mod scope_context;

use super::{MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource};
use crate::waterfall::{RequestPayload, points};
use cordis_core::{WaterfallDecision, WaterfallListener};

/// 固定返回预设条目的替身检索源。
struct Fixed(Vec<MemoryEntry>);

impl MemorySource for Fixed {
    fn recall(&self, _session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry> {
        self.0.iter().take(limit).cloned().collect()
    }
}

fn entry(key: &str, value: &str) -> MemoryEntry {
    MemoryEntry {
        key: key.into(),
        value: value.into(),
        tags: String::new(),
        updated_at: 0,
        scope: String::new(),
    }
}

fn cfg_lock(c: MemoryInjectConfig) -> std::sync::Arc<std::sync::RwLock<MemoryInjectConfig>> {
    std::sync::Arc::new(std::sync::RwLock::new(c))
}

fn payload_value() -> serde_json::Value {
    serde_json::to_value(RequestPayload {
        messages: Vec::new(),
        model: None,
        params: serde_json::Value::Object(serde_json::Map::new()),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: None,
        tree_directory: None,
    })
    .unwrap()
}

fn inject(
    source: std::sync::Arc<dyn MemorySource>,
    cfg: std::sync::Arc<std::sync::RwLock<MemoryInjectConfig>>,
) -> MemoryInject {
    MemoryInject::new(source, cfg)
}

async fn run(inject: &MemoryInject, key: &str) -> Option<RequestPayload> {
    match inject.handle(key, &payload_value()).await {
        WaterfallDecision::Replace(v) => Some(serde_json::from_value(v).unwrap()),
        WaterfallDecision::Continue => None,
        _ => panic!("unexpected decision"),
    }
}

/// 带会话身份的请求。
async fn run_session(inject: &MemoryInject, session_id: &str) -> Option<RequestPayload> {
    let mut v = payload_value();
    v["session_id"] = serde_json::json!(session_id);
    match inject.handle(points::REQUEST, &v).await {
        WaterfallDecision::Replace(v) => Some(serde_json::from_value(v).unwrap()),
        WaterfallDecision::Continue => None,
        _ => panic!("unexpected decision"),
    }
}
