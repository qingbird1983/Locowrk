//! 匹配测试：id 归一化 / host 与别名打分 / 无定位不借池 / 真快照守卫
//! （预设别名表 × 随包快照漂移防线）。

use super::*;
use crate::llm::catalog::{parse_str, CatalogSource};

fn snap(text: &str) -> CatalogSnapshot {
    parse_str(text, CatalogSource::Bundled).unwrap()
}

const FIXTURE: &str = r#"{
  "deepseek": {"api": "https://api.deepseek.com", "models": {
    "deepseek-flash": {"name": "Flash"},
    "deepseek-chat-2024-08-06": {"name": "Old"}}},
  "openai": {"models": {"gpt-4o-2024-08-06": {"name": "GPT-4o"}, "gpt-4o-latest": {"name": "Latest"}}},
  "moonshotai": {"api": "https://api.moonshot.ai/v1", "models": {"kimi-k2": {"name": "K2"}}},
  "zhipuai": {"api": "https://open.bigmodel.cn/api/paas/v4", "models": {"glm-5": {"name": "GLM-5"}}},
  "lmstudio": {"api": "http://127.0.0.1:1234/v1", "models": {"qwen3-8b": {"name": "Qwen3"}}}
}"#;

#[test]
fn normalize_strips_dates_latest_region_and_path() {
    assert_eq!(normalize_id("gpt-4o-2024-08-06"), "gpt-4o");
    assert_eq!(
        normalize_id("claude-3-5-sonnet-20241022"),
        "claude-3-5-sonnet"
    );
    assert_eq!(
        normalize_id("deepseek/deepseek-flash-latest"),
        "deepseek-flash"
    );
    assert_eq!(normalize_id("llama3@together"), "llama3");
    assert_eq!(normalize_id("DeepSeek-Flash"), "deepseek-flash");
    assert_eq!(normalize_id("glm-5-"), "glm-5");
    // 非日期形态不动
    assert_eq!(normalize_id("qwen3-8b"), "qwen3-8b");
    assert_eq!(normalize_id("70b"), "70b");
}

#[test]
fn host_of_extracts_host_keeping_port() {
    assert_eq!(host_of("https://api.deepseek.com"), "api.deepseek.com");
    assert_eq!(host_of("http://localhost:1234/v1"), "localhost:1234");
    assert_eq!(
        host_of("https://ark.cn-beijing.volces.com/api/v3"),
        "ark.cn-beijing.volces.com"
    );
}

#[test]
fn alias_host_locates_provider_and_matches_id() {
    let s = snap(FIXTURE);
    // 智谱预设 host → zhipuai（别名表 +100，快照 api 同 host 双保险）
    let hit = lookup(
        &s,
        "openai",
        Some("https://open.bigmodel.cn/api/paas/v4"),
        "glm-5",
    )
    .unwrap();
    assert_eq!(hit.provider_key, "zhipuai");
    // 月之暗面预设 host（api.moonshot.cn）≠ 快照 api（api.moonshot.ai）：
    // 纯别名表命中
    let hit = lookup(&s, "openai", Some("https://api.moonshot.cn/v1"), "kimi-k2").unwrap();
    assert_eq!(hit.provider_key, "moonshotai");
}

#[test]
fn snapshot_api_host_matches_without_alias() {
    let s = snap(FIXTURE);
    // 别名表没有的端点，快照 api host 兜底（+80）
    let hit = lookup(
        &s,
        "openai",
        Some("https://api.deepseek.com/v1"),
        "deepseek-flash",
    )
    .unwrap();
    assert_eq!(hit.provider_key, "deepseek");
}

#[test]
fn normalized_id_hits_date_and_latest_variants() {
    let s = snap(FIXTURE);
    let hit = lookup(
        &s,
        "openai",
        Some("https://api.openai.com/v1"),
        "gpt-4o-2024-08-06",
    )
    .unwrap();
    assert_eq!(hit.model_id, "gpt-4o-2024-08-06");
    // 请求名带日期戳，目录里是裸名 → 归一化命中
    let hit = lookup(
        &s,
        "openai",
        Some("https://api.openai.com/v1"),
        "gpt-4o-2030-01-01",
    )
    .unwrap();
    assert_eq!(hit.model_id, "gpt-4o-2024-08-06");
    let hit = lookup(
        &s,
        "openai",
        Some("https://api.openai.com/v1"),
        "gpt-4o-latest",
    )
    .unwrap();
    assert_eq!(hit.model_id, "gpt-4o-latest");
}

#[test]
fn localhost_port_maps_to_lmstudio() {
    let s = snap(FIXTURE);
    let hit = lookup(&s, "openai", Some("http://localhost:1234/v1"), "qwen3-8b").unwrap();
    assert_eq!(hit.provider_key, "lmstudio");
    let hit = lookup(&s, "openai", Some("http://127.0.0.1:1234/v1"), "qwen3-8b").unwrap();
    assert_eq!(hit.provider_key, "lmstudio");
}

#[test]
fn no_provider_hit_means_no_match_no_borrow_pool() {
    let s = snap(FIXTURE);
    // 中转站 host 无定位：即使模型 id 全网同名也不借池（宁缺勿假）
    assert!(lookup(&s, "openai", Some("https://relay.example.com/v1"), "glm-5").is_none());
    // 定位了但没有该模型：同样 None
    assert!(lookup(&s, "openai", Some("https://api.deepseek.com"), "gpt-4o").is_none());
    // ollama 本地：目录不适用
    assert!(lookup(&s, "ollama", Some("http://localhost:11434"), "qwen3:8b").is_none());
    // base_url 缺失
    assert!(lookup(&s, "openai", None, "glm-5").is_none());
}

/// 真快照守卫：别名表每个键都必须存在于随包快照（models.dev 供应商
/// 改名/移除时编译期外唯一能抓到的点——防映射静默悬空）。
#[test]
fn alias_table_keys_exist_in_bundled_snapshot() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/models.dev/api.json");
    let text = std::fs::read_to_string(&path).expect("随包快照应在仓库内");
    let s = parse_str(&text, CatalogSource::Bundled).expect("快照应可解析");
    let providers: std::collections::HashSet<&str> =
        s.models.keys().map(|(p, _)| p.as_str()).collect();
    for (host, key) in PRESET_HOST_PROVIDER {
        assert!(
            providers.contains(key),
            "别名表 {host} → {key} 在快照里不存在（models.dev 改名？更新映射表）"
        );
    }
}

/// 真快照冒烟：主力预设 host + 现役模型应命中（deepseek/智谱/openai）。
#[test]
fn bundled_snapshot_hits_mainline_models() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/models.dev/api.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let s = parse_str(&text, CatalogSource::Bundled).unwrap();
    for (base, model) in [
        ("https://api.deepseek.com", "deepseek-flash"),
        ("https://open.bigmodel.cn/api/paas/v4", "glm-5"),
        ("https://api.openai.com/v1", "gpt-4o"),
        (
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "gemini-2.5-pro",
        ),
    ] {
        let hit = lookup(&s, "openai", Some(base), model);
        assert!(hit.is_some(), "{model} @ {base} 应命中快照");
    }
}
