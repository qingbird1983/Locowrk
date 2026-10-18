//! 连通测试两路（§8.3）：`llm_test_model` 吃**已存档位**，`llm_test_custom` 吃**表单当前值**
//! （未保存也可测，v2.143 —— 此前误接 `llm_probe_models` 与「获取列表」同功能，用户目检实抓）。
//! 两路都走 `crate::llm::test_model` 的最小 ping：五分类错误 + 延迟如实。

use serde::Serialize;
use tauri::State;

use crate::llm::keyring;
use crate::state::AppState;

/// 逐模型连通测试（§8.3）：最小请求 ping，五分类错误 + 延迟如实。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelTestResult {
    pub ok: bool,
    pub category: Option<String>,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

#[tauri::command]
pub async fn llm_test_model(
    profile_id: String,
    model: String,
    state: State<'_, AppState>,
) -> Result<ModelTestResult, String> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err("模型名为空，无法测试".into());
    }
    let cfg = state.settings.llm_config();
    let p = cfg
        .profile(&profile_id)
        .ok_or_else(|| format!("档位不存在: {profile_id}"))?;
    let flat = cfg.flat_profile(p);
    match crate::llm::test_model(&flat, &model).await {
        Ok(ms) => Ok(ModelTestResult {
            ok: true,
            category: None,
            message: None,
            latency_ms: Some(ms),
        }),
        Err(f) => Ok(ModelTestResult {
            ok: false,
            category: Some(f.category().to_string()),
            message: Some(f.message()),
            latency_ms: None,
        }),
    }
}

/// 表单连通测试（v2.143）：编辑模态「测试连接」——此前误接 llm_probe_models
/// （与「获取列表」同功能，用户目检实抓）。改走 test_model 最小 ping，且吃
/// **表单当前值**（未保存也可测）：Key 留空回落 `profile_id` 已存 Key、
/// headers 取已存档位（与 llm_probe_models 同口径）。
#[tauri::command]
pub async fn llm_test_custom(
    kind: String,
    base_url: Option<String>,
    api_key: Option<String>,
    profile_id: Option<String>,
    model: String,
    state: State<'_, AppState>,
) -> Result<ModelTestResult, String> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err("模型名为空，无法测试".into());
    }
    let cfg = state.settings.llm_config();
    let stored = profile_id.as_deref().and_then(|id| cfg.profile(id));
    let key = match api_key.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => Some(k.to_string()),
        None => stored.and_then(keyring::resolve),
    };
    let headers = stored.map(|p| p.headers.clone()).unwrap_or_default();
    let flat = crate::settings::LlmSettings {
        provider: kind,
        model: model.clone(),
        base_url,
        api_key: key,
        window_tokens: None,
        headers,
        network_proxy: cfg.network_proxy.clone(),
    };
    match crate::llm::test_model(&flat, &model).await {
        Ok(ms) => Ok(ModelTestResult {
            ok: true,
            category: None,
            message: None,
            latency_ms: Some(ms),
        }),
        Err(f) => Ok(ModelTestResult {
            ok: false,
            category: Some(f.category().to_string()),
            message: Some(f.message()),
            latency_ms: None,
        }),
    }
}
