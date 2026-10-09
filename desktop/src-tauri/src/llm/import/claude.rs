//! claude-code 解析器（批 F）：`~/.claude/settings.json` 的 `env` 块
//! （+ `settings.local.json` 浅合并，local 覆盖全局）。
//! cc-switch 的 claude 行 `settings_config = {env:{...}}` 同形复用。

use serde_json::Value;

use super::{candidate_id, ImportModel, ParsedProvider, SOURCE_CLAUDE};

/// claude `env` 解析产物（base_url / key / 去重模型清单 / 剥尾缀 notes）。
pub(crate) struct ClaudeEnv {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub models: Vec<String>,
    pub notes: Vec<String>,
}

/// 从 claude `env` 对象解析（无模型 = None）。
pub(crate) fn parse_claude_env(env: &Value) -> Option<ClaudeEnv> {
    let get = |k: &str| {
        env.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let key = get("ANTHROPIC_AUTH_TOKEN").or_else(|| get("ANTHROPIC_API_KEY"));
    let base_url = get("ANTHROPIC_BASE_URL");
    // Claude Code 的模型值可带「[1M]」这类上下文提示尾缀——真实模型 id 是
    // '[' 之前的部分（导入落我们的档位，尾缀剥掉 + note 说明）。
    let strip = |s: &str| s.split('[').next().unwrap_or(s).trim().to_string();
    let mut models: Vec<String> = Vec::new();
    let mut notes = Vec::new();
    let mut push = |raw: Option<&str>| {
        if let Some(m) = raw {
            let id = strip(m);
            if !id.is_empty() && !models.iter().any(|x| x == &id) {
                if m.contains('[') {
                    let note = format!("模型「{m}」剥上下文尾缀导入为 {id}");
                    if !notes.contains(&note) {
                        notes.push(note);
                    }
                }
                models.push(id);
            }
        }
    };
    push(get("ANTHROPIC_MODEL"));
    push(get("ANTHROPIC_DEFAULT_SONNET_MODEL"));
    push(get("ANTHROPIC_DEFAULT_OPUS_MODEL"));
    push(get("ANTHROPIC_DEFAULT_HAIKU_MODEL"));
    push(get("ANTHROPIC_DEFAULT_FABLE_MODEL"));
    if models.is_empty() {
        return None;
    }
    Some(ClaudeEnv {
        base_url: base_url.map(str::to_string),
        api_key: key.map(str::to_string),
        models,
        notes,
    })
}

/// claude-code 主扫描：settings.json + settings.local.json。缺 Key 或缺
/// 模型 = None（上游记 errors）。
pub(crate) fn scan_claude_code(home: &std::path::Path) -> Option<ParsedProvider> {
    let paths = super::ImportPaths::from_home(home);
    let read_json = |p: &std::path::Path| -> Option<Value> {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    };
    let global = read_json(&paths.claude_settings)?;
    let mut env = global.get("env").cloned().unwrap_or(Value::Null);
    if let Some(local) = read_json(&paths.claude_local) {
        if let (Some(dst), Some(src)) = (
            env.as_object_mut(),
            local.get("env").and_then(Value::as_object),
        ) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
    }
    let e = parse_claude_env(&env)?;
    let model = e.models.first().cloned().unwrap_or_default();
    Some(ParsedProvider {
        source: SOURCE_CLAUDE,
        id: candidate_id(SOURCE_CLAUDE, "main"),
        label: "Claude Code".into(),
        kind: "anthropic".into(),
        base_url: e.base_url,
        api_key: e.api_key,
        model,
        models: e
            .models
            .into_iter()
            .map(|id| ImportModel {
                id,
                context_window: None,
            })
            .collect(),
        notes: e.notes,
    })
}
