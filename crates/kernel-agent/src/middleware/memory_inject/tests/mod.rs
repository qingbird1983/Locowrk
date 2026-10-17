//! memory_inject 单测：注入格式 / 配置开关 / 空表降级 / 超长截断 /
//! 追加语义 / 非 REQUEST 点不碰。
//! 批 F-16 目录化：共享假件与跑批入口留本文件，用例按面分簇见 `render` /
//! `prefetch` / `scope_context`；Q2（MR §4.1）加 `project_index`
//! （项目记忆索引段的落点、缺席条件与随预取钉死）；Q9（ML §3.3/§3.4）加
//! `learn`（相关性词表的切分与下传形状——四信号打分在宿主侧测）。

mod learn;
mod prefetch;
mod project_index;
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

/// 跑一环并回传载荷。`None` = 本环没改过一个字节。
///
/// 判据原来取自决策（`Replace` = 改过），台账 T2 之后注入环就地写、收束恒为
/// `Continue`，只能比进出的载荷。输入由 `to_value(RequestPayload)` 造 ⇒ round-trip
/// 幂等 ⇒ 字节差只可能是业务改写。
async fn pass(
    inject: &MemoryInject,
    key: &str,
    input: serde_json::Value,
) -> Option<RequestPayload> {
    let mut v = input.clone();
    match inject.handle(key, &mut v).await {
        WaterfallDecision::Continue => {}
        other => panic!("unexpected decision {other:?}"),
    }
    (v != input).then(|| serde_json::from_value(v).unwrap())
}

async fn run(inject: &MemoryInject, key: &str) -> Option<RequestPayload> {
    pass(inject, key, payload_value()).await
}

/// 带会话身份的请求。
async fn run_session(inject: &MemoryInject, session_id: &str) -> Option<RequestPayload> {
    let mut v = payload_value();
    v["session_id"] = serde_json::json!(session_id);
    pass(inject, points::REQUEST, v).await
}
