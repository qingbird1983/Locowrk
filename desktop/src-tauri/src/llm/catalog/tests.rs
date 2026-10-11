//! 目录解析与归一化测试：快照裁剪（deepseek 真实形状样本）/ 梯子归一
//! （toggle、effort、none、未知值）/ 状态投影 / 空快照兜底。

use super::*;
use serde_json::json;

const DEEPSEEK_FLASH: &str = r#"{
  "id": "deepseek-flash",
  "name": "DeepSeek V4.1 Flash",
  "reasoning": true,
  "reasoning_options": [
    {"type": "toggle"},
    {"type": "effort", "values": ["low", "high", "max"]}
  ],
  "tool_call": true,
  "modalities": {"input": ["text", "image"], "output": ["text"]},
  "limit": {"context": 1000000, "output": 393216},
  "cost": {"input": 0.15, "output": 0.6, "cache_read": 0.003}
}"#;

#[test]
fn parse_trims_expected_fields() {
    let mv: Value = serde_json::from_str(DEEPSEEK_FLASH).unwrap();
    let m = parse_model("deepseek", "deepseek-flash", &mv);
    assert_eq!(m.provider_key, "deepseek");
    assert_eq!(m.display_name.as_deref(), Some("DeepSeek V4.1 Flash"));
    assert_eq!(m.context_window, Some(1_000_000));
    assert_eq!(m.max_output, Some(393_216));
    assert_eq!(m.input_modalities, vec!["text", "image"]);
    // toggle + effort 归一：disabled 打头，effort 按语义序
    assert_eq!(m.reasoning_options, vec!["disabled", "low", "high", "max"]);
    assert_eq!(m.tool_call, Some(true));
    let p = m.pricing.unwrap();
    assert_eq!(p.input, Some(0.15));
    assert_eq!(p.output, Some(0.6));
    assert_eq!(p.cache_read, Some(0.003));
}

#[test]
fn parse_missing_fields_stay_none() {
    let mv = json!({"id": "x", "name": "X"});
    let m = parse_model("p", "x", &mv);
    assert_eq!(m.context_window, None);
    assert_eq!(m.max_output, None);
    assert!(m.input_modalities.is_empty());
    assert!(m.reasoning_options.is_empty());
    assert_eq!(m.tool_call, None);
    assert_eq!(m.pricing, None);
}

#[test]
fn reasoning_toggle_only_yields_disabled_enabled() {
    let v = json!([{"type": "toggle"}]);
    assert_eq!(normalize_reasoning(Some(&v)), vec!["disabled", "enabled"]);
}

#[test]
fn reasoning_effort_none_maps_to_disabled() {
    let v = json!([{"type": "effort", "values": ["none", "minimal", "low", "medium", "high", "xhigh", "max"]}]);
    assert_eq!(
        normalize_reasoning(Some(&v)),
        vec!["disabled", "minimal", "low", "medium", "high", "xhigh", "max"]
    );
}

#[test]
fn reasoning_unknown_values_keep_order_after_known() {
    let v = json!([{"type": "effort", "values": ["ultra", "high"]}]);
    assert_eq!(normalize_reasoning(Some(&v)), vec!["high", "ultra"]);
}

#[test]
fn parse_str_counts_providers_and_models() {
    let text = r#"{
      "deepseek": {"id": "deepseek", "api": "https://api.deepseek.com",
        "models": {"deepseek-flash": {"name": "Flash"}, "deepseek-v4-pro": {"name": "Pro"}}},
      "empty": {"id": "empty", "models": {}},
      "nomodels": {"id": "nomodels"}
    }"#;
    let s = parse_str(text, CatalogSource::Bundled).unwrap();
    assert_eq!(s.models.len(), 2);
    assert_eq!(
        s.api_index.get("deepseek").map(String::as_str),
        Some("https://api.deepseek.com")
    );
    let st = s.status();
    assert_eq!(st.source, CatalogSource::Bundled);
    assert_eq!(st.models, 2);
    assert_eq!(st.providers, 1);
}

#[test]
fn store_load_failure_degrades_to_empty() {
    // 目录名带测试名：两用例同 pid 目录会互删（并行线程竞态，2026-10-09 满
    // 跑实抓）——各用独立目录
    let dir = crate::testutil::work_dir("lw-catalog-missing");
    let path = dir.join("missing.json");
    let store = CatalogStore::new(path);
    let st = store.snapshot().status();
    assert_eq!(st.models, 0);
    assert_eq!(st.source, CatalogSource::Bundled);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn store_lazy_loads_from_file() {
    let dir = crate::testutil::work_dir("lw-catalog-lazy");
    let path = dir.join("api.json");
    std::fs::write(
        &path,
        r#"{"deepseek": {"models": {"deepseek-flash": {"name": "Flash"}}}}"#,
    )
    .unwrap();
    let store = CatalogStore::new(path.clone());
    let hit = store.lookup_model("openai", Some("https://api.deepseek.com"), "deepseek-flash");
    assert!(hit.is_some(), "快照文件应被惰性加载并命中");
    assert_eq!(store.snapshot().status().models, 1);
    std::fs::remove_dir_all(&dir).ok();
}
