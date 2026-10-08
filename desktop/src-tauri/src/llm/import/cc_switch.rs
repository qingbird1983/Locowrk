//! cc-switch 解析器（批 F）：`~/.cc-switch/cc-switch.db`（SQLite，
//! `providers` 表）——它是**容器**：`settings_config` 按 `app_type` 分派
//! 给 claude / codex / opencode 三个解析器（claude = `{env:{...}}`、
//! codex = `{auth, config}`、opencode = provider 对象）。legacy
//! `config.json`（v2）存在才兜底读（v3+ 已迁库）。
//!
//! 密钥纪律同模块级：读到的 Key 只在 `ParsedProvider` 里流转，IPC 面
//! 剥成 `has_key`。

use rusqlite::OpenFlags;
use serde_json::Value;

use super::{claude, codex, opencode, ImportModel, ParsedProvider};

/// cc-switch 主扫描：Err = 库打不开/表缺（多数机器根本没装 = 正常缺席，
/// 上游记 errors 不算错态）；Ok(空) = 库在但无可用 provider 行。
pub(crate) fn scan_cc_switch(home: &std::path::Path) -> Result<Vec<ParsedProvider>, String> {
    let paths = super::ImportPaths::from_home(home);
    if !paths.cc_switch_db.exists() {
        return Err("未发现 ~/.cc-switch/cc-switch.db".into());
    }
    let conn = rusqlite::Connection::open_with_flags(
        &paths.cc_switch_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("cc-switch.db 打开失败: {e}"))?;
    let mut stmt = conn
        .prepare("SELECT id, app_type, name, settings_config FROM providers ORDER BY app_type, sort_index")
        .map_err(|e| format!("providers 表读取失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3).unwrap_or_default(),
            ))
        })
        .map_err(|e| format!("providers 表读取失败: {e}"))?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        let (id, app_type, name, config_text) = row;
        let cfg: Value = serde_json::from_str(&config_text).unwrap_or(Value::Null);
        match app_type.as_str() {
            "claude" => {
                let Some(env) = cfg.get("env").and_then(claude::parse_claude_env) else {
                    continue;
                };
                let model = env.models.first().cloned().unwrap_or_default();
                out.push(ParsedProvider {
                    source: super::SOURCE_CC_SWITCH,
                    id: super::candidate_id(super::SOURCE_CC_SWITCH, &id),
                    label: format!("cc-switch ({name})"),
                    kind: "anthropic".into(),
                    base_url: env.base_url,
                    api_key: env.api_key,
                    model,
                    models: env
                        .models
                        .into_iter()
                        .map(|m| ImportModel { id: m, context_window: None })
                        .collect(),
                    notes: env.notes,
                });
            }
            "codex" => {
                let key = cfg
                    .get("auth")
                    .and_then(|a| a.get("OPENAI_API_KEY"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let text = cfg.get("config").and_then(Value::as_str).unwrap_or("");
                if let Some(mut p) = codex::from_parts(key, text) {
                    p.source = super::SOURCE_CC_SWITCH;
                    p.id = super::candidate_id(super::SOURCE_CC_SWITCH, &id);
                    p.label = format!("cc-switch ({name})");
                    out.push(p);
                }
            }
            "opencode" => {
                // settings_config = 单个 provider 对象（npm/name/options/models）
                if let Some(mut p) = opencode::parse_provider(&id, &cfg, None) {
                    p.source = super::SOURCE_CC_SWITCH;
                    p.id = super::candidate_id(super::SOURCE_CC_SWITCH, &id);
                    p.label = format!("cc-switch ({name})");
                    out.push(p);
                }
            }
            // gemini / claude-desktop 等形态本应用无对应 kind——不发明映射
            _ => {}
        }
    }
    Ok(out)
}
