//! models.rs 解析层测试（网络层薄封装不测——端点契约由实机走查覆盖）。

use super::{join_version, parse_models_data, parse_ollama_tags, sweep_candidates};
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

// ---- 端点扫描候选（批 C §8.2）：现值剔除 + ≤3 候选 + 裸域名补 scheme ----

#[test]
fn sweep_candidates_bare_domain_relays() {
    // 中转站贴裸域名：现值 join_version 不带 scheme（首探必败于此），
    // 候选全部补 https://——含 /v1（新形态）+ /api/v1 + /api
    assert_eq!(
        sweep_candidates("relay.example.com"),
        vec![
            "https://relay.example.com/v1".to_string(),
            "https://relay.example.com/api/v1".to_string(),
            "https://relay.example.com/api".to_string()
        ]
    );
}

#[test]
fn sweep_candidates_pathed_input() {
    // 用户已带 /api：现值归一为 /api/v1（刚失败，剔除），试 /v1 与 /api
    assert_eq!(
        sweep_candidates("https://gw.example.com/api"),
        vec![
            "https://gw.example.com/v1".to_string(),
            "https://gw.example.com/api".to_string()
        ]
    );
}

#[test]
fn sweep_candidates_dedup_and_versioned_input() {
    // 已带 /v1：唯一候选 origin/api/v1、/api；/v1 与现值重复不出现
    assert_eq!(
        sweep_candidates("https://api.x.ai/v1"),
        vec![
            "https://api.x.ai/api/v1".to_string(),
            "https://api.x.ai/api".to_string()
        ]
    );
}

#[test]
fn sweep_candidates_bare_local_gets_http() {
    // 裸 IPv4/localhost：http:// 补全（本地服务常态），/v1 为胜出候选
    assert_eq!(
        sweep_candidates("127.0.0.1:18900"),
        vec![
            "http://127.0.0.1:18900/v1".to_string(),
            "http://127.0.0.1:18900/api/v1".to_string(),
            "http://127.0.0.1:18900/api".to_string()
        ]
    );
    assert_eq!(
        sweep_candidates("localhost:11434"),
        vec![
            "http://localhost:11434/v1".to_string(),
            "http://localhost:11434/api/v1".to_string(),
            "http://localhost:11434/api".to_string()
        ]
    );
}

#[test]
fn sweep_candidates_rejects_non_url() {
    assert!(sweep_candidates("").is_empty());
    assert!(sweep_candidates("   ").is_empty());
    // 非法 URL / 非 http(s) scheme：不扫（不把凭据发去未知形态的主机）
    assert!(sweep_candidates(":::not a url").is_empty());
    assert!(sweep_candidates("ftp://files.example.com").is_empty());
}

#[test]
fn sweep_candidates_keeps_port() {
    assert_eq!(
        sweep_candidates("http://127.0.0.1:8080"),
        vec![
            "http://127.0.0.1:8080/api/v1".to_string(),
            "http://127.0.0.1:8080/api".to_string()
        ]
    );
}
