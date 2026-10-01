//! embed 单测：响应解析 / 配置解析 / 冷却降级（真实 HTTP 失败路径）。

use super::*;
use crate::settings::EmbeddingSettings;
use serde_json::json;

#[test]
fn parse_embeddings_sorts_by_index() {
    let resp = json!({
        "data": [
            { "index": 1, "embedding": [0.0, 1.0] },
            { "index": 0, "embedding": [1.0, 0.0] }
        ]
    });
    let out = parse_embeddings(&resp, 2).unwrap();
    assert_eq!(out[0], vec![1.0, 0.0]);
    assert_eq!(out[1], vec![0.0, 1.0]);
}

#[test]
fn parse_embeddings_rejects_bad_payloads() {
    // 条数不符
    let resp = json!({ "data": [{ "index": 0, "embedding": [1.0] }] });
    assert!(parse_embeddings(&resp, 2).is_err());
    // 缺 index
    let resp = json!({ "data": [{ "embedding": [1.0] }] });
    assert!(parse_embeddings(&resp, 1).is_err());
    // 缺 embedding
    let resp = json!({ "data": [{ "index": 0 }] });
    assert!(parse_embeddings(&resp, 1).is_err());
    // 缺 data
    assert!(parse_embeddings(&json!({}), 1).is_err());
}

#[test]
fn from_settings_needs_model_and_resolvable_base() {
    let llm = EmbeddingSettingsLlmView {
        provider: "ollama".into(),
        base_url: None,
    };
    // 模型没填 = None
    assert!(EmbedClient::from_settings(&EmbeddingSettings::default(), &llm).is_none());
    // 模型填了 + ollama 兜底端点 = Some
    let e = EmbeddingSettings {
        base_url: None,
        model: Some("nomic-embed-text".into()),
        api_key: None,
    };
    let c = EmbedClient::from_settings(&e, &llm).unwrap();
    assert_eq!(c.base_url, "http://localhost:11434");
    assert_eq!(c.model, "nomic-embed-text");
    // 尾斜杠归一
    let e = EmbeddingSettings {
        base_url: Some("http://embed.local/".into()),
        model: Some("m".into()),
        api_key: None,
    };
    let llm = EmbeddingSettingsLlmView {
        provider: "openai".into(),
        base_url: Some("http://llm.local".into()),
    };
    assert_eq!(
        EmbedClient::from_settings(&e, &llm).unwrap().base_url,
        "http://embed.local"
    );
}

#[tokio::test]
async fn cooled_embed_fails_closed_and_cooldowns() {
    let shared = SharedEmbed::new();
    let port = shared.port();
    // 未配置 = Err，且不进冷却（配置到来后立即可用）
    assert!(port.embed(&["x".into()]).await.is_err());
    shared.set(Some(EmbedClient {
        base_url: "http://127.0.0.1:9".into(), // discard 端口，连接必拒
        model: "m".into(),
        api_key: None,
    }));
    // 第一次真实失败 → 进冷却
    assert!(port.embed(&["x".into()]).await.is_err());
    // 冷却期内再试 = 快速 Err（不再打网络）
    let e = port.embed(&["x".into()]).await.unwrap_err();
    assert!(e.contains("冷却"));
}

#[tokio::test]
async fn empty_batch_skips_http() {
    let shared = SharedEmbed::new();
    shared.set(Some(EmbedClient {
        base_url: "http://127.0.0.1:9".into(),
        model: "m".into(),
        api_key: None,
    }));
    let port = shared.port();
    assert_eq!(port.embed(&[]).await.unwrap(), Vec::<Vec<f32>>::new());
}
