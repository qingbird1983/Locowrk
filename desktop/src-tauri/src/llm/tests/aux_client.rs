//! 摘要直呼出口的**取档**判据（S4 刀③a）：压缩线的 `SummaryClient` 只认主模型，
//! 记忆管理器的服务档与模型名不再参与。
//!
//! 为什么不测"发出去的请求长什么样"：那一段与改前逐字相同（`complete_inner` 一行未动），
//! 本刀改的只有"从设置里取哪一档"。取档错了在行为面上是**另一台服务器**收到请求——
//! 假客户端数不出这件事，而下面两条读的是真设置算出来的 `LlmSettings`。

use super::*;

use crate::llm::AppLlmSummarizer;

/// 两档提供商 + 记忆管理器指到第二档：主档与记忆档在端点/模型上全不同，
/// 取错一眼能看见（同一个 base_url 的夹具测不出路由）。
fn store_with_two_routes(tag: &str) -> Arc<SettingsStore> {
    let root = temp_root(tag);
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({
            "llm": {
                "active": "main",
                "providers": [
                    {
                        "id": "main",
                        "label": "主档",
                        "kind": "openai",
                        "baseUrl": "https://main.example/v1",
                        "apiKey": "sk-main",
                        "model": "main-model",
                        "windowTokens": 131072,
                        "enabled": true
                    },
                    {
                        "id": "mem",
                        "label": "记忆档",
                        "kind": "openai",
                        "baseUrl": "https://mem.example/v1",
                        "apiKey": "sk-mem",
                        "model": "mem-model",
                        "windowTokens": 131072,
                        "enabled": true
                    }
                ]
            },
            // 持久化键沿用历史名（`settings/memory/mod.rs` 的 serde rename）：
            // 这里故意把它填满，证明压缩线**不认**这两格。
            "memory": {
                "recorderModel": "mem-model",
                "recorderProvider": "mem"
            }
        })
        .to_string(),
    )
    .unwrap();
    Arc::new(SettingsStore::open(&root).unwrap())
}

/// 压缩摘要取主档：记忆管理器配了服务档与模型名也不换档。
#[test]
fn summary_takes_the_main_route_and_ignores_the_memory_manager_profile() {
    let store = store_with_two_routes("aux-main-route");
    let summarizer = AppLlmSummarizer::new(Arc::clone(&store));
    let cfg = summarizer.cfg_main(None);
    assert_eq!(
        cfg.base_url.as_deref(),
        Some("https://main.example/v1"),
        "摘要必须走主模型端点，实测 {}",
        cfg.base_url.as_deref().unwrap_or("<None>")
    );
    assert_eq!(cfg.model, "main-model", "摘要必须用主模型");
}

/// `model` 参数只换模型名：端点与档位仍是主档的（压缩设置里的模型覆盖
/// 不该把请求带去记忆管理器那台服务器）。
#[test]
fn summary_model_override_keeps_the_main_endpoint() {
    let store = store_with_two_routes("aux-model-override");
    let summarizer = AppLlmSummarizer::new(store);
    let cfg = summarizer.cfg_main(Some("override-model"));
    assert_eq!(cfg.model, "override-model");
    assert_eq!(
        cfg.base_url.as_deref(),
        Some("https://main.example/v1"),
        "只换模型名，不换端点"
    );
    // 空串 = 不覆盖（设置里那枚模型留空 = 跟主模型）。
    let cfg = summarizer.cfg_main(Some("   "));
    assert_eq!(cfg.model, "main-model", "空白模型名等于没有覆盖");
}
