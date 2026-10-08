//! codex 解析器（批 F）：`~/.codex/config.toml`（`model_provider` +
//! `[model_providers.<id>]`）+ `~/.codex/auth.json`（`OPENAI_API_KEY`）。
//! cc-switch 的 codex 行 `{auth, config}` 同构复用（config 字段就是
//! config.toml 文本）。
//!
//! **如实标注不拦**：`wire_api = "responses"` 的端点走 OpenAI Responses
//! API，本应用只说 chat/completions——兼容性未验证，进 notes 由用户预览
//! 自行取舍（国内主流中转多同时伺服两种）。

use serde_json::Value;

use super::{candidate_id, ImportModel, ParsedProvider, SOURCE_CODEX};

/// config.toml 文本 → (provider 键, base_url, wire_api, 模型清单)。
pub(crate) fn parse_codex_toml(text: &str) -> Option<(String, Option<String>, String, Vec<String>)> {
    let cfg: toml::Value = toml::from_str(text).ok()?;
    let provider_id = cfg
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .unwrap_or("openai")
        .to_string();
    let mut models: Vec<String> = Vec::new();
    let mut push = |v: Option<&str>| {
        if let Some(m) = v.map(str::trim).filter(|s| !s.is_empty()) {
            if !models.iter().any(|x| x == m) {
                models.push(m.to_string());
            }
        }
    };
    push(cfg.get("model").and_then(toml::Value::as_str));
    push(cfg.get("review_model").and_then(toml::Value::as_str));
    if models.is_empty() {
        return None;
    }
    let table = cfg
        .get("model_providers")
        .and_then(|p| p.get(&provider_id));
    let base_url = table
        .and_then(|t| t.get("base_url"))
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    let wire_api = table
        .and_then(|t| t.get("wire_api"))
        .and_then(toml::Value::as_str)
        .unwrap_or("chat")
        .to_string();
    Some((provider_id, base_url, wire_api, models))
}

pub(crate) fn from_parts(
    key: Option<String>,
    config_text: &str,
) -> Option<ParsedProvider> {
    let (provider_id, base_url, wire_api, models) = parse_codex_toml(config_text)?;
    let mut notes = Vec::new();
    if wire_api != "chat" {
        notes.push(format!(
            "codex 端点 wire_api={wire_api}（Responses API），chat/completions 兼容性未验证"
        ));
    }
    let label = if provider_id == "openai" {
        "Codex (OpenAI)".to_string()
    } else {
        format!("Codex ({provider_id})")
    };
    Some(ParsedProvider {
        source: SOURCE_CODEX,
        id: candidate_id(SOURCE_CODEX, "main"),
        label,
        kind: "openai".into(),
        base_url,
        api_key: key,
        model: models.first().cloned().unwrap_or_default(),
        models: models
            .into_iter()
            .map(|id| ImportModel { id, context_window: None })
            .collect(),
        notes,
    })
}

/// codex 主扫描：Err = 配置读不了；Ok(None) = 配置在但缺 Key/模型。
pub(crate) fn scan_codex(
    home: &std::path::Path,
) -> Result<Option<ParsedProvider>, String> {
    let paths = super::ImportPaths::from_home(home);
    if !paths.codex_config.exists() {
        return Err("未发现 ~/.codex/config.toml".into());
    }
    let text = std::fs::read_to_string(&paths.codex_config)
        .map_err(|e| format!("config.toml 读取失败: {e}"))?;
    let key: Option<String> = std::fs::read_to_string(&paths.codex_auth)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("OPENAI_API_KEY").and_then(Value::as_str).map(str::to_string));
    Ok(from_parts(key, &text))
}
