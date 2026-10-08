//! 导入解析器单测（纯函数 + home 注入，不碰真实用户目录）：四源形态
//! fixture 覆盖 + 密钥不出 IPC 面 + apply 去重/跳过。

use std::path::Path;

use serde_json::{json, Value};

use super::{claude, codex, opencode, apply, scan, scan_full, SOURCE_CLAUDE};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("locowrk-import-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

#[test]
fn claude_env_parses_and_strips_suffix() {
    let env = json!({
        "ANTHROPIC_AUTH_TOKEN": "sk-test",
        "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
        "ANTHROPIC_MODEL": "deepseek-v4-flash[1M]",
        "ANTHROPIC_DEFAULT_SONNET_MODEL": "deepseek-v4-flash",
        "ANTHROPIC_DEFAULT_OPUS_MODEL": "deepseek-v4-flash[1M]"
    });
    let e = claude::parse_claude_env(&env).expect("parsed");
    assert_eq!(e.base_url.as_deref(), Some("https://api.deepseek.com/anthropic"));
    assert_eq!(e.api_key.as_deref(), Some("sk-test"));
    assert_eq!(e.models, vec!["deepseek-v4-flash".to_string()]);
    assert!(e.notes.iter().any(|n| n.contains("剥上下文尾缀")));
}

#[test]
fn claude_scan_merges_local_and_reports_missing_key_via_none() {
    let home = temp_home("claude");
    write(
        &home.join(".claude/settings.json"),
        r#"{"env": {"ANTHROPIC_BASE_URL": "https://x.example", "ANTHROPIC_MODEL": "glm-5"}}"#,
    );
    write(
        &home.join(".claude/settings.local.json"),
        r#"{"env": {"ANTHROPIC_AUTH_TOKEN": "sk-local"}}"#,
    );
    let p = claude::scan_claude_code(&home).expect("parsed");
    assert_eq!(p.kind, "anthropic");
    assert_eq!(p.api_key.as_deref(), Some("sk-local"));
    assert_eq!(p.model, "glm-5");
    // 无 Key 无模型 = None（上游记 errors）；临时目录即清
    let empty = temp_home("claude-empty");
    assert!(claude::scan_claude_code(&empty).is_none());
    let _ = std::fs::remove_dir_all(&empty);
}

#[test]
fn codex_parses_toml_and_notes_wire_api() {
    let toml_text = r#"
model_provider = "custom"
model = "deepseek-v4-flash"
review_model = "gpt-5.5"

[model_providers.custom]
name = "deepseek"
base_url = "https://api.deepseek.com"
wire_api = "responses"
"#;
    let p = codex::from_parts(Some("sk-codex".into()), toml_text).expect("parsed");
    assert_eq!(p.kind, "openai");
    assert_eq!(p.base_url.as_deref(), Some("https://api.deepseek.com"));
    assert_eq!(p.model, "deepseek-v4-flash");
    assert_eq!(p.models.len(), 2);
    assert!(p.notes.iter().any(|n| n.contains("responses")));
}

#[test]
fn opencode_parses_providers_and_current_model() {
    let cfg = json!({
        "model": "cherry-gateway/opencode:deepseek-v4-flash",
        "provider": {
            "cherry-gateway": {
                "npm": "@ai-sdk/openai-compatible",
                "name": "cherry-gateway",
                "options": { "apiKey": "sk-oc", "baseURL": "http://127.0.0.1:23333/v1" },
                "models": {
                    "opencode:deepseek-v4-flash": { "limit": { "context": 1048576 } },
                    "other": {}
                }
            }
        }
    });
    let mut out = Vec::new();
    for (pid, pv) in cfg.get("provider").and_then(Value::as_object).expect("map") {
        if let Some(p) = opencode::parse_provider(pid, pv, None) {
            out.push(p);
        }
    }
    opencode::apply_current_model(&mut out, cfg.get("model").and_then(Value::as_str));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].model, "opencode:deepseek-v4-flash");
    assert_eq!(out[0].models[0].context_window, Some(1048576));
}

#[test]
fn scan_strips_keys_from_ipc_surface() {
    let home = temp_home("scan");
    write(
        &home.join(".claude/settings.json"),
        r#"{"env": {"ANTHROPIC_AUTH_TOKEN": "sk-secret", "ANTHROPIC_MODEL": "glm-5"}}"#,
    );
    let outcome = scan(&home);
    assert!(outcome.errors.iter().all(|e| !e.contains("sk-secret")));
    for c in &outcome.candidates {
        assert!(!c.notes.iter().any(|n| n.contains("sk-secret")));
    }
    let claude_hit = outcome
        .candidates
        .iter()
        .find(|c| c.source == SOURCE_CLAUDE)
        .expect("claude candidate");
    assert!(claude_hit.has_key);
}

#[test]
fn apply_imports_and_dedups() {
    let home = temp_home("apply");
    write(
        &home.join(".claude/settings.json"),
        r#"{"env": {"ANTHROPIC_AUTH_TOKEN": "sk-a", "ANTHROPIC_BASE_URL": "https://a.example", "ANTHROPIC_MODEL": "glm-5"}}"#,
    );
    let mut cfg = crate::settings::LlmConfig::default();
    let (parsed, _) = scan_full(&home);
    let id = parsed[0].id.clone();

    let report = apply(&home, std::slice::from_ref(&id), &mut cfg);
    assert_eq!(report.imported.len(), 1);
    assert_eq!(cfg.providers.len(), 1);
    // 密钥落档（后端面），label 带来源尾缀
    assert_eq!(cfg.providers[0].api_key.as_deref(), Some("sk-a"));
    assert!(cfg.providers[0].label.contains(SOURCE_CLAUDE));

    // 同端点同模型重复导入 = 跳过
    let report2 = apply(&home, &[id], &mut cfg);
    assert!(report2.imported.is_empty());
    assert_eq!(report2.skipped.len(), 1);

    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn apply_unknown_id_reports_skip() {
    let home = temp_home("apply-unknown");
    let mut cfg = crate::settings::LlmConfig::default();
    let report = apply(&home, &["claude-code:ghost".to_string()], &mut cfg);
    assert_eq!(report.skipped.len(), 1);
    let _ = std::fs::remove_dir_all(&home);
}
