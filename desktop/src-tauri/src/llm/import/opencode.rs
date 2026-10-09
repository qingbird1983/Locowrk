//! opencode 解析器（批 F）：`~/.config/opencode/opencode.json` 的
//! `provider.<id>` 块（`options.{apiKey,baseURL}` + `models{}`）+ 顶层
//! `model = "<provider>/<model-id>"`。Key 缺席回落
//! `~/.local/share/opencode/auth.json`（provider id 键）。
//! cc-switch 的 opencode 行 = 单个 provider 对象同形复用。

use serde_json::Value;

use super::{candidate_id, ImportModel, ParsedProvider, SOURCE_OPENCODE};

/// 单 provider 对象 → 候选（无端点 / npm 形态不兼容 = None，缺 Key 照出
/// ——has_key=false 让用户预览时自行取舍）。
pub(crate) fn parse_provider(
    pid: &str,
    pv: &Value,
    fallback_key: Option<String>,
) -> Option<ParsedProvider> {
    let options = pv.get("options");
    let base_url = options
        .and_then(|o| o.get("baseURL"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)?;
    let npm = pv.get("npm").and_then(Value::as_str).unwrap_or("");
    let mut notes = Vec::new();
    if !npm.is_empty() && npm != "@ai-sdk/openai-compatible" {
        notes.push(format!(
            "opencode 适配器 {npm} 非 openai-compatible，兼容性未验证"
        ));
    }
    let key = options
        .and_then(|o| o.get("apiKey"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(fallback_key);
    let mut models: Vec<ImportModel> = Vec::new();
    if let Some(map) = pv.get("models").and_then(Value::as_object) {
        for (mid, mv) in map {
            models.push(ImportModel {
                id: mid.clone(),
                context_window: mv
                    .get("limit")
                    .and_then(|l| l.get("context"))
                    .and_then(Value::as_u64),
            });
        }
    }
    if models.is_empty() {
        return None;
    }
    let display = pv
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(pid);
    Some(ParsedProvider {
        source: SOURCE_OPENCODE,
        id: candidate_id(SOURCE_OPENCODE, pid),
        label: format!("opencode ({display})"),
        kind: "openai".into(),
        base_url: Some(base_url),
        api_key: key,
        // 当前模型（顶层 "pid/mid"）命中本 provider 时排首位；否则清单序。
        model: String::new(),
        models,
        notes,
    })
}

/// 当前模型串归位：`model = "cherry-gateway/opencode:deepseek-v4-flash"`
/// = provider 段 / 模型段（首个 '/' 切分；模型 id 自身可含 ':'）。
pub(crate) fn apply_current_model(parsed: &mut [ParsedProvider], current: Option<&str>) {
    let Some(cur) = current.map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    let Some((pid, mid)) = cur.split_once('/') else {
        return;
    };
    if let Some(p) = parsed
        .iter_mut()
        .find(|p| p.id == candidate_id(SOURCE_OPENCODE, pid))
    {
        if p.models.iter().any(|m| m.id == mid) {
            p.model = mid.to_string();
        }
    }
}

/// opencode 主扫描：Err = 配置读不了；Ok(空) = 无带端点的 provider。
pub(crate) fn scan_opencode(home: &std::path::Path) -> Result<Vec<ParsedProvider>, String> {
    let paths = super::ImportPaths::from_home(home);
    if !paths.opencode_json.exists() {
        return Err("未发现 ~/.config/opencode/opencode.json".into());
    }
    let text = std::fs::read_to_string(&paths.opencode_json)
        .map_err(|e| format!("opencode.json 读取失败: {e}"))?;
    let cfg: Value =
        serde_json::from_str(&text).map_err(|e| format!("opencode.json 解析失败: {e}"))?;
    // auth.json 回落键表（provider id → key）
    let auth: Value = std::fs::read_to_string(&paths.opencode_auth)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);
    let auth_key = |pid: &str| {
        auth.get(pid)
            .and_then(|e| e.get("key"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let mut out: Vec<ParsedProvider> = Vec::new();
    if let Some(map) = cfg.get("provider").and_then(Value::as_object) {
        for (pid, pv) in map {
            if let Some(mut p) = parse_provider(pid, pv, auth_key(pid)) {
                if p.api_key.is_none() {
                    p.notes
                        .push("未发现 API Key（opencode.json 与 auth.json 都没有）".into());
                }
                out.push(p);
            }
        }
    }
    apply_current_model(&mut out, cfg.get("model").and_then(Value::as_str));
    Ok(out)
}
