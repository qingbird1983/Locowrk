//! models.rs 解析层测试（网络层薄封装不测——端点契约由实机走查覆盖）。

use super::{join_version, parse_models_data, parse_ollama_tags};
use serde_json::{json, Value};

#[test]
fn ollama_tags_parse_names() {
    let resp = json!({
        "models": [
            { "name": "qwen3:8b", "size": 1 },
            { "name": "llama3.1:latest" },
            { "digest": "sha256:x" } // 无 name 的条目跳过
        ]
    });
    assert_eq!(
        parse_ollama_tags(&resp).unwrap(),
        vec!["qwen3:8b".to_string(), "llama3.1:latest".to_string()]
    );
}

#[test]
fn ollama_tags_rejects_missing_array() {
    let err = parse_ollama_tags(&json!({ "error": "oops" })).unwrap_err();
    assert!(err.contains("models"));
}

#[test]
fn openai_shape_parse_ids() {
    let resp = json!({
        "data": [
            { "id": "deepseek-chat", "owned_by": "deepseek" },
            { "id": "deepseek-flash" },
            { "object": "model" } // 无 id 的条目跳过
        ]
    });
    assert_eq!(
        parse_models_data(&resp).unwrap(),
        vec!["deepseek-chat".to_string(), "deepseek-flash".to_string()]
    );
}

#[test]
fn openai_shape_rejects_foreign_payload() {
    let err = parse_models_data(&Value::Array(vec![])).unwrap_err();
    assert!(err.contains("data"));
}

// ---- join_version（㉚）：缺版本段补 /v1，已带 /v<数字> 原样 ----

#[test]
fn join_version_inserts_v1_only_when_missing() {
    assert_eq!(
        join_version("https://api.deepseek.com"),
        "https://api.deepseek.com/v1"
    );
    assert_eq!(join_version(" https://api.x.ai "), "https://api.x.ai/v1");
    // 已带版本段（任意数字）原样——智谱 /v4、方舟 /v3、OpenAI /v1
    assert_eq!(
        join_version("https://open.bigmodel.cn/api/paas/v4"),
        "https://open.bigmodel.cn/api/paas/v4"
    );
    assert_eq!(
        join_version("https://ark.cn-beijing.volces.com/api/v3/"),
        "https://ark.cn-beijing.volces.com/api/v3"
    );
    assert_eq!(
        join_version("https://api.openai.com/v1"),
        "https://api.openai.com/v1"
    );
    // 尾斜杠在归一前剥掉，不产生 //
    assert_eq!(
        join_version("https://api.moonshot.cn/v1/"),
        "https://api.moonshot.cn/v1"
    );
    // v 后非纯数字不是版本段（vm2024 之类不误判）
    assert_eq!(
        join_version("https://x.example.com/vm"),
        "https://x.example.com/vm/v1"
    );
    // 裸主机名（无路径段）
    assert_eq!(join_version("localhost:11434"), "localhost:11434/v1");
}
