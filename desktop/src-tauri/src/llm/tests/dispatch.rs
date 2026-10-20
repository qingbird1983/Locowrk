//! 端点与设置分发：base_url 归一化 / 缺模型回落 demo / 已配模型缺 Key 快速失败 /
//! 会话级覆盖的四层生效链。

use super::*;

// ---------- 端点与设置分发 ----------

/// 端点归一：缺版本段补 /v1；ollama 落官方兼容端点 /v1；智谱 /v4、方舟
/// /v3 原样保留（LLM.md 覆盖表）。
#[test]
fn endpoint_normalizes_join_version() {
    let cfg = |provider: &str, base: Option<&str>| crate::settings::LlmSettings {
        provider: provider.into(),
        model: "m".into(),
        base_url: base.map(Into::into),
        api_key: None,
        window_tokens: None,
        ..Default::default()
    };
    assert_eq!(
        CompatLlm::endpoint(&cfg("openai", Some("https://api.deepseek.com"))),
        "https://api.deepseek.com/v1/chat/completions"
    );
    assert_eq!(
        CompatLlm::endpoint(&cfg("openai", Some("https://open.bigmodel.cn/api/paas/v4"))),
        "https://open.bigmodel.cn/api/paas/v4/chat/completions"
    );
    assert_eq!(
        CompatLlm::endpoint(&cfg("anthropic", None)),
        "https://api.anthropic.com/v1/chat/completions"
    );
    assert_eq!(
        CompatLlm::endpoint(&cfg("ollama", Some("http://localhost:11434"))),
        "http://localhost:11434/v1/chat/completions"
    );
}

/// model 未配置 → 演示替身（开箱即用，不碰网络）。
#[tokio::test]
async fn empty_model_falls_back_to_demo() {
    let root = temp_root("demo-fallback");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    let llm = AppLlm::new(Arc::clone(&store));

    let resp = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "hi".into(),
                parts: Vec::new(),
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(resp.content.contains("演示模型"));
    assert!(resp.tool_calls.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// 配好 model 且缺 Key → 真实路径的快速失败（提示去设置面板填）。
#[tokio::test]
async fn configured_model_without_key_fails_fast() {
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return; // 环境里有 Key 时该前置检查不触发，跳过
    }
    let root = temp_root("real-path");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    store
        .update(
            None,
            None,
            None,
            Some(crate::settings::LlmUpdate {
                provider: Some("openai".into()),
                model: Some("gpt-4o-mini".into()),
                base_url: None,
                api_key: None,
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let llm = AppLlm::new(Arc::clone(&store));

    let err = llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: "hi".into(),
                parts: Vec::new(),
            }],
            ..Default::default()
        })
        .await
        .unwrap_err();
    let LlmError::Backend(msg) = err;
    assert!(msg.contains("OPENAI_API_KEY"), "got: {msg}");
    let _ = std::fs::remove_dir_all(&root);
}

/// v2.58 §3.30 会话级模型覆盖解析：覆盖命中 = 整档换路由（kind/端点/Key
/// 随档位）+ 模型名替换；未命中会话 / 无注入 = 设置缺省；决策环
/// `req.model`（waterfall 契约）优先级最高。
#[test]
fn effective_cfg_resolves_session_override() {
    use crate::model_override::ModelOverrideStore;
    use kernel_agent::LlmRequest;

    let root = temp_root("override-cfg");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    // 活动档 default（openai/m1）+ 第二档 p2（anthropic/m2 自带端点）
    store
        .update_llm_config(|cfg| {
            cfg.providers.push(crate::settings::ProviderProfile {
                id: "default".into(),
                label: "default".into(),
                kind: "openai".into(),
                model: "m1".into(),
                enabled: true,
                ..Default::default()
            });
            cfg.providers.push(crate::settings::ProviderProfile {
                id: "p2".into(),
                label: "P2".into(),
                kind: "anthropic".into(),
                base_url: Some("https://relay.example.com".into()),
                api_key: Some("sk-2".into()),
                model: "m2".into(),
                window_tokens: Some(200_000),
                enabled: true,
                models: Vec::new(),
                ..Default::default()
            });
            cfg.active = "default".into();
            Ok(())
        })
        .unwrap();

    let dir = temp_root("override-sidecar");
    let overrides = Arc::new(ModelOverrideStore::new(dir.clone()));
    let llm = AppLlm::with_overrides(Arc::clone(&store), Arc::clone(&overrides));
    let req = |sid: Option<&str>, model: Option<&str>| LlmRequest {
        session_id: sid.map(Into::into),
        model: model.map(Into::into),
        ..Default::default()
    };

    // 未覆盖会话 = 设置缺省（活动档）
    let cfg = llm.effective_cfg(&req(Some("s1"), None));
    assert_eq!(
        (cfg.provider.as_str(), cfg.model.as_str()),
        ("openai", "m1")
    );

    // 覆盖命中 = 换档路由 + 模型名
    overrides.set("s1", Some(("p2", "m2-x"))).unwrap();
    let cfg = llm.effective_cfg(&req(Some("s1"), None));
    assert_eq!(cfg.provider, "anthropic");
    assert_eq!(cfg.model, "m2-x");
    assert_eq!(cfg.base_url.as_deref(), Some("https://relay.example.com"));
    assert_eq!(cfg.api_key.as_deref(), Some("sk-2"));
    assert_eq!(cfg.window_tokens, Some(200_000));

    // 决策环 model 改写压过会话覆盖
    let cfg = llm.effective_cfg(&req(Some("s1"), Some("m-decision")));
    assert_eq!(cfg.model, "m-decision");

    // 无注入存储 = 永远缺省（测试路径）
    let plain = AppLlm::new(Arc::clone(&store));
    let cfg = plain.effective_cfg(&req(Some("s1"), None));
    assert_eq!(cfg.model, "m1");
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&dir).ok();
}
