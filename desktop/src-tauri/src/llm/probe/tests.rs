//! probe 单测：/api/show 两代响应形态的窗口字段解析（不发真实网络请求）。

use super::*;
use serde_json::json;

#[test]
fn parses_top_level_context_length() {
    let resp = json!({ "modelfile": "x", "context_length": 131_072 });
    assert_eq!(parse_context_length(&resp), Some(131_072));
}

#[test]
fn parses_model_info_arch_prefixed_key() {
    let resp = json!({
        "model_info": {
            "general.architecture": "llama",
            "llama.context_length": 32_768,
            "llama.block_count": 32
        }
    });
    assert_eq!(parse_context_length(&resp), Some(32_768));
}

#[test]
fn missing_field_is_none() {
    assert_eq!(parse_context_length(&json!({ "details": {} })), None);
    assert_eq!(parse_context_length(&json!({ "model_info": {} })), None);
    // 顶层优先于 model_info（两代形态并存时取新字段）
    let resp = json!({ "context_length": 8, "model_info": { "q.context_length": 9 } });
    assert_eq!(parse_context_length(&resp), Some(8));
}
