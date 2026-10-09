//! 从其他工具导入（批 F，LLM.md「生态」节）：claude-code / codex /
//! opencode / cc-switch 四解析器（纯读）+ 预览确认 + 写入
//! `ProviderProfile`。
//!
//! 密钥纪律：解析出的 Key **不进 IPC**——扫描面只回 `has_key` 布尔；
//! 导入应用（apply）在后端重跑同一解析取 Key 落档（明文入 settings，
//! keyring 惰性迁移照批 C 通道收口）。

mod cc_switch;
mod claude;
mod codex;
mod opencode;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use crate::settings::{LlmConfig, ProviderModel, ProviderProfile};
use std::collections::BTreeMap;

/// 来源标识（IPC `source` 字段 + 候选 id 前缀）。
pub const SOURCE_CLAUDE: &str = "claude-code";
pub const SOURCE_CODEX: &str = "codex";
pub const SOURCE_OPENCODE: &str = "opencode";
pub const SOURCE_CC_SWITCH: &str = "cc-switch";

/// 解析产物（内含 Key，只在后端流转）。
#[derive(Debug, Clone, Default)]
pub(crate) struct ParsedProvider {
    pub source: &'static str,
    /// 候选稳定 id：`{source}:{key}`（apply 按它回配）。
    pub id: String,
    pub label: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: String,
    pub models: Vec<ImportModel>,
    pub notes: Vec<String>,
}

/// 候选清单里的一个模型（id + 可选上下文窗口，导入落 `ProviderModel`）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportModel {
    pub id: String,
    pub context_window: Option<u64>,
}

/// 扫描候选（**无 Key**——`has_key` 布尔代替，IPC 面）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub source: String,
    pub id: String,
    pub label: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub model: String,
    pub models: Vec<ImportModel>,
    pub has_key: bool,
    pub notes: Vec<String>,
}

/// 全源扫描结果：候选 + 各源失败记因（如实逐源报错，不拖垮其它源）。
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOutcome {
    pub candidates: Vec<ImportCandidate>,
    pub errors: Vec<String>,
}

/// 导入应用结果（逐条如实：导入 / 跳过带因）。
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}

/// 全源扫描（带 Key 的内部形态）。`home` 注入（测试隔离；生产 = 用户主目录）。
pub(crate) fn scan_full(home: &Path) -> (Vec<ParsedProvider>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    if let Some(p) = claude::scan_claude_code(home) {
        out.push(p);
    } else {
        errors.push(format!(
            "{SOURCE_CLAUDE}: 未发现可用配置（~/.claude/settings.json 无 Key 或缺模型）"
        ));
    }
    match codex::scan_codex(home) {
        Ok(p) => {
            if let Some(p) = p {
                out.push(p);
            } else {
                errors.push(format!("{SOURCE_CODEX}: 配置存在但缺 Key 或模型"));
            }
        }
        Err(e) => errors.push(format!("{SOURCE_CODEX}: {e}")),
    }
    match opencode::scan_opencode(home) {
        Ok(list) => {
            if list.is_empty() {
                errors.push(format!("{SOURCE_OPENCODE}: 未发现带端点的 provider 配置"));
            }
            out.extend(list);
        }
        Err(e) => errors.push(format!("{SOURCE_OPENCODE}: {e}")),
    }
    match cc_switch::scan_cc_switch(home) {
        Ok(list) => out.extend(list),
        Err(e) => errors.push(format!("{SOURCE_CC_SWITCH}: {e}")),
    }
    (out, errors)
}

/// IPC 扫描面：剥 Key → `has_key`。
pub fn scan(home: &Path) -> ScanOutcome {
    let (parsed, errors) = scan_full(home);
    ScanOutcome {
        candidates: parsed
            .into_iter()
            .map(|p| ImportCandidate {
                source: p.source.to_string(),
                id: p.id,
                label: p.label,
                kind: p.kind,
                base_url: p.base_url,
                model: p.model,
                models: p.models,
                has_key: p.api_key.is_some(),
                notes: p.notes,
            })
            .collect(),
        errors,
    }
}

/// 导入选中项（后端重扫回配 id 取 Key → 新档追加）。id 未命中 / 缺 Key /
/// 已有同 kind+端点+模型的档位 = 跳过并记因，不中断其余。
pub fn apply(home: &Path, ids: &[String], cfg: &mut LlmConfig) -> ApplyReport {
    let (parsed, _) = scan_full(home);
    let mut report = ApplyReport::default();
    for id in ids {
        let Some(p) = parsed.iter().find(|p| &p.id == id) else {
            report
                .skipped
                .push(format!("{id}: 候选不存在（源文件可能已变化，请重新扫描）"));
            continue;
        };
        let Some(key) = p.api_key.as_deref().filter(|k| !k.trim().is_empty()) else {
            report
                .skipped
                .push(format!("{id}: 未发现 API Key，无法导入"));
            continue;
        };
        let norm = |u: &str| u.trim().trim_end_matches('/').to_string();
        if cfg.providers.iter().any(|x| {
            x.kind == p.kind
                && x.base_url.as_deref().map(norm) == p.base_url.as_deref().map(norm)
                && x.model == p.model
        }) {
            report
                .skipped
                .push(format!("{id}: 已存在同类型同端点同模型的档位，跳过"));
            continue;
        }
        // 候选 label 已带来源前缀（如「cc-switch (stepfun)」）时不重复
        // 追加尾缀，避免「cc-switch (stepfun)（cc-switch）」双标。
        let label = if p.label.to_lowercase().contains(p.source) {
            p.label.clone()
        } else {
            format!("{}（{}）", p.label, p.source)
        };
        let profile = ProviderProfile {
            id: crate::settings::new_profile_id(),
            label,
            kind: p.kind.clone(),
            base_url: p.base_url.clone(),
            api_key: Some(key.trim().to_string()),
            model: p.model.clone(),
            window_tokens: None,
            enabled: true,
            models: p
                .models
                .iter()
                .map(|m| ProviderModel {
                    id: m.id.clone(),
                    alias: None,
                    context_window: m.context_window,
                    max_output: None,
                    max_output_source: None,
                    supports_image: None,
                    available_for_subagents: None,
                })
                .collect(),
            headers: BTreeMap::new(),
            key_storage: None,
        };
        report.imported.push(profile.label.clone());
        cfg.providers.push(profile);
    }
    report
}

/// 候选 id 组装（各解析器共用约定）。
pub(crate) fn candidate_id(source: &str, key: &str) -> String {
    format!("{source}:{key}")
}

/// 路径束：home 下四源位置（测试与生产同源）。
pub(crate) struct ImportPaths {
    pub claude_settings: PathBuf,
    pub claude_local: PathBuf,
    pub codex_config: PathBuf,
    pub codex_auth: PathBuf,
    pub opencode_json: PathBuf,
    pub opencode_auth: PathBuf,
    pub cc_switch_db: PathBuf,
}

impl ImportPaths {
    pub fn from_home(home: &Path) -> Self {
        Self {
            claude_settings: home.join(".claude").join("settings.json"),
            claude_local: home.join(".claude").join("settings.local.json"),
            codex_config: home.join(".codex").join("config.toml"),
            codex_auth: home.join(".codex").join("auth.json"),
            opencode_json: home.join(".config").join("opencode").join("opencode.json"),
            opencode_auth: home
                .join(".local")
                .join("share")
                .join("opencode")
                .join("auth.json"),
            cc_switch_db: home.join(".cc-switch").join("cc-switch.db"),
        }
    }
}
