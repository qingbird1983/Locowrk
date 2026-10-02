//! memory_inject 单测：注入格式 / 配置开关 / 空表降级 / 超长截断 /
//! 追加语义 / 非 REQUEST 点不碰。

use super::{
    ExperienceEntry, ExperienceSource, MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource,
};
use crate::waterfall::{RequestPayload, points};
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::json;

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
    })
    .unwrap()
}

fn inject(
    source: std::sync::Arc<dyn MemorySource>,
    cfg: std::sync::Arc<std::sync::RwLock<MemoryInjectConfig>>,
) -> MemoryInject {
    MemoryInject::new(source, None, cfg)
}

async fn run(inject: &MemoryInject, key: &str) -> Option<RequestPayload> {
    match inject.handle(key, &payload_value()).await {
        WaterfallDecision::Replace(v) => Some(serde_json::from_value(v).unwrap()),
        WaterfallDecision::Continue => None,
        _ => panic!("unexpected decision"),
    }
}

#[tokio::test]
async fn injects_block_into_system() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("user.lang", "zh-CN")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("<persistent_memory>"));
    assert!(system.contains("- user.lang = zh-CN"));
    assert!(!system.contains("truncated"));
}

#[tokio::test]
async fn top_k_limits_entries() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![
            entry("a", "1"),
            entry("b", "2"),
            entry("c", "3"),
        ])),
        cfg_lock(MemoryInjectConfig {
            enabled: true,
            top_k: 2,
        }),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("- a = 1"));
    assert!(system.contains("- b = 2"));
    assert!(!system.contains("- c = 3"));
}

#[tokio::test]
async fn disabled_or_zero_top_k_is_noop() {
    for cfg in [
        MemoryInjectConfig {
            enabled: false,
            top_k: 5,
        },
        MemoryInjectConfig {
            enabled: true,
            top_k: 0,
        },
    ] {
        let inject = inject(
            std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
            cfg_lock(cfg),
        );
        assert!(run(&inject, points::REQUEST).await.is_none());
    }
}

#[tokio::test]
async fn empty_recall_is_noop() {
    let inject = inject(
        std::sync::Arc::new(Fixed(Vec::new())),
        cfg_lock(MemoryInjectConfig::default()),
    );
    assert!(run(&inject, points::REQUEST).await.is_none());
}

#[tokio::test]
async fn long_values_truncated_with_marker() {
    let long = "x".repeat(600);
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("big", &long)])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("…(truncated)"));
    // 恰好 500 个 x + 标记，而不是 600 个
    assert_eq!(system.matches('x').count(), 500);
}

#[tokio::test]
async fn tags_rendered_when_present() {
    let mut e = entry("k", "v");
    e.tags = "pref".into();
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![e])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    assert!(out.system.unwrap().contains(" [tags: pref]"));
}

#[tokio::test]
async fn appends_to_existing_system() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let mut payload = payload_value();
    payload["system"] = json!("base instructions");
    match inject.handle(points::REQUEST, &payload).await {
        WaterfallDecision::Replace(v) => {
            let req: RequestPayload = serde_json::from_value(v).unwrap();
            let system = req.system.unwrap();
            assert!(system.starts_with("base instructions"));
            assert!(system.contains("<persistent_memory>"));
        }
        _ => panic!("expected replace"),
    }
}

#[tokio::test]
async fn non_request_key_is_noop() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    assert!(run(&inject, points::PRE_STEP).await.is_none());
}

/// 固定返回预设经历的替身检索源（⑬）。
struct FixedExp(Vec<ExperienceEntry>);

#[async_trait::async_trait]
impl ExperienceSource for FixedExp {
    async fn recall(
        &self,
        _session_id: Option<&str>,
        _query: &str,
        limit: usize,
    ) -> Vec<ExperienceEntry> {
        self.0.iter().take(limit).cloned().collect()
    }
}

fn exp(content: &str, created_at: i64) -> ExperienceEntry {
    ExperienceEntry {
        content: content.into(),
        session_id: "s1".into(),
        created_at,
    }
}

/// 带最近用户消息的 payload（经历检索的 query 来源）。
fn payload_with_user(text: &str) -> serde_json::Value {
    let mut v = payload_value();
    v["messages"] = json!([{ "role": "user", "content": text }]);
    v
}

#[tokio::test]
async fn experiences_injected_when_source_present() {
    let inject = MemoryInject::new(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        Some(std::sync::Arc::new(FixedExp(vec![exp(
            "chose rusqlite",
            1_700_000_000,
        )]))),
        cfg_lock(MemoryInjectConfig::default()),
    );
    match inject
        .handle(points::REQUEST, &payload_with_user("向量库选型"))
        .await
    {
        WaterfallDecision::Replace(v) => {
            let req: RequestPayload = serde_json::from_value(v).unwrap();
            let system = req.system.unwrap();
            assert!(system.contains("<persistent_memory>"));
            assert!(system.contains("<session_experiences>"));
            assert!(system.contains("(2023-11-14) chose rusqlite"));
        }
        _ => panic!("expected replace"),
    }
}

#[tokio::test]
async fn experiences_skipped_when_absent_or_no_query() {
    // 未提供经历源：只有事实块
    let inject = MemoryInject::new(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        None,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    assert!(out.system.unwrap().contains("<persistent_memory>"));
    // 有源但会话没有用户消息（query 空）→ 不检索、无经历块
    let inject = MemoryInject::new(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        Some(std::sync::Arc::new(FixedExp(vec![exp("x", 0)]))),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    assert!(!out.system.unwrap().contains("<session_experiences>"));
}

#[test]
fn date_of_formats_utc() {
    assert_eq!(super::date_of(0), "1970-01-01");
    assert_eq!(super::date_of(1_700_000_000), "2023-11-14");
}
