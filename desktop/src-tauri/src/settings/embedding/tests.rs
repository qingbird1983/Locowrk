//! embedding 区单测：端点兜底次序与就绪判定（本区值 > provider 兜底 >
//! 主模型端点）。

use super::*;

#[test]
fn base_url_resolution_order() {
    let e = EmbeddingSettings {
        base_url: Some("http://embed.local".into()),
        model: Some("m".into()),
        api_key: None,
    };
    // 本区显式值永远优先
    assert_eq!(
        e.resolved_base_url("openai", Some("http://llm.local")),
        Some("http://embed.local".into())
    );
    // ollama 兜底本机缺省
    let e = EmbeddingSettings::default();
    assert_eq!(
        e.resolved_base_url("ollama", Some("http://llm.local")),
        Some(OLLAMA_DEFAULT_BASE.into())
    );
    // 其余 provider 回落主模型端点；两者皆无 = None
    assert_eq!(
        e.resolved_base_url("openai", Some("http://llm.local")),
        Some("http://llm.local".into())
    );
    assert_eq!(e.resolved_base_url("openai", None), None);
}

#[test]
fn readiness_requires_model_and_endpoint() {
    // 模型没填 = 不就绪（端点可兜底也没用）
    let e = EmbeddingSettings::default();
    assert!(!e.is_ready("ollama", None));
    // 模型空串 = 未填
    let e = EmbeddingSettings {
        base_url: None,
        model: Some(String::new()),
        api_key: None,
    };
    assert!(!e.is_ready("ollama", None));
    // 模型 + 可兜底端点 = 就绪
    let e = EmbeddingSettings {
        base_url: None,
        model: Some("nomic-embed-text".into()),
        api_key: None,
    };
    assert!(e.is_ready("ollama", None));
    assert!(!e.is_ready("openai", None));
}
