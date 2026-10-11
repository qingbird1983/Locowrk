//! LLM 区的**旧格式迁移**簇（审计 F5 拆自原 `settings/llm.rs`）：`migrate_flat_llm`
//! 把 v2.58 之前的平面 llm 区落成单档 `default`（DESIGN §13.1，一次幂等）。
//!
//! 单独成簇的理由：它是唯一**读磁盘 raw JSON** 的一段（其余各簇都工作在反序列化之后），
//! 与合成/IPC 面毫无共享代码——原来却隔 400 行同处一文件。

/// 旧平面 llm 区 → 单档 `default`（v2.58 档位制，DESIGN §13.1）。幂等：
/// 新格式 llm 区无 `provider` 键，检测即安全。旧值（含 apiKey 原值）
/// 全量保留；label 用 kind 名（用户可在设置里改名）。
pub fn migrate_flat_llm(raw: &mut serde_json::Value) {
    let Some(llm) = raw.get_mut("llm").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if !llm.contains_key("provider") {
        return;
    }
    let mut old = serde_json::Map::new();
    for k in ["provider", "model", "baseUrl", "apiKey", "windowTokens"] {
        if let Some(v) = llm.remove(k) {
            old.insert(k.to_string(), v);
        }
    }
    let kind = old
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("openai")
        .to_string();
    *llm = serde_json::json!({
        "active": "default",
        "providers": [{
            "id": "default",
            "label": kind,
            "kind": kind,
            "baseUrl": old.get("baseUrl").cloned().unwrap_or(serde_json::Value::Null),
            "apiKey": old.get("apiKey").cloned().unwrap_or(serde_json::Value::Null),
            "model": old.get("model").cloned().unwrap_or(serde_json::json!("")),
            "windowTokens": old.get("windowTokens").cloned().unwrap_or(serde_json::Value::Null),
        }]
    })
    .as_object()
    .cloned()
    .expect("profile object");
}
