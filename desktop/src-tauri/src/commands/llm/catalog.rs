//! 模型能力目录三件（批 A，供应商稿 §4.5）+ 前缀断裂诊断（M6.1）：状态 / 在线刷新
//! （只进内存，永不落盘）/ 单模型查询，加最近一步 LLM 请求的分块 hash 链比对。
//! 诊断读的是 `AppState.llm_diagnoser`，开关关时恒 None（记录面就不在）。

use tauri::State;

use crate::llm::catalog::CatalogStatus;
use crate::state::AppState;

// ---- 模型能力目录（批 A，供应商稿 §4.5）----
/// 目录状态（首访惰性加载随包快照）。
#[tauri::command]
pub fn llm_catalog_status(state: State<'_, AppState>) -> CatalogStatus {
    state.catalog.snapshot().status()
}

/// 在线刷新 models.dev（只进内存，永不落盘；失败旧快照原样保留）。
#[tauri::command]
pub async fn llm_catalog_refresh(state: State<'_, AppState>) -> Result<CatalogStatus, String> {
    state.catalog.refresh().await
}

/// 单模型目录查询（供应商定位 → 归一化 id 精确命中；None = 能力未知）。
#[tauri::command]
pub fn llm_lookup_model(
    kind: String,
    base_url: Option<String>,
    model: String,
    state: State<'_, AppState>,
) -> Option<crate::llm::catalog::CatalogModel> {
    state
        .catalog
        .lookup_model(&kind, base_url.as_deref(), &model)
}

/// 前缀断裂诊断（M6.1，KV v2 §7.4）：会话最近一步 LLM 请求的分块 hash
/// 链比对结果（breaks 空 = 前缀健康纯追加）。开关
/// `developer.cacheDiagnostics` 关时 AppLlm 不记录 = 恒 None。
#[tauri::command]
pub async fn llm_cache_diagnose(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::llm::diagnose::BreakReport>, String> {
    Ok(state.llm_diagnoser.last_of(&session_id))
}
