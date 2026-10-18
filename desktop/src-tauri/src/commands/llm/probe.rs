//! 表单探测（§8.2）：清单或错误 + 扫描自动补全的端点 + 推荐三选（批 F ③：目录数据
//! 驱动的确定性预选，全未命中 = 列表序前 3 保底）。Key 空缺回落已存档位（凭据库双读），
//! 与表单连通测试同口径。

use serde::Serialize;
use tauri::State;

use crate::llm::catalog::CatalogStore;
use crate::llm::{keyring, models};
use crate::state::AppState;

/// 表单探测结果（§8.2）：清单或错误 + 扫描自动补全的端点（与传入不同
/// 时 Some，表单字段未再编辑时自动采纳）+ 推荐三选（批 F：目录数据驱动
/// 的确定性预选 ≤3，前端添加路径自动勾选）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProbeOutcome {
    pub models: Vec<String>,
    pub error: Option<String>,
    pub effective_base_url: Option<String>,
    pub recommended: Vec<String>,
}

/// 推荐三选（批 F ③）：探测清单逐个查目录，命中面进 `recommend` 纯函
/// 数（toolCall 必备 / 族分散 / 超贵降权）；全未命中 = 列表序前 3 保底。
fn recommend_probe_models(
    catalog: &CatalogStore,
    kind: &str,
    base_url: Option<&str>,
    models: &[String],
) -> Vec<String> {
    let candidates: Vec<crate::llm::recommend::RecCandidate> = models
        .iter()
        .map(|id| {
            let hit = catalog.lookup_model(kind, base_url, id);
            crate::llm::recommend::RecCandidate {
                id: id.clone(),
                tool_call: hit.as_ref().and_then(|h| h.tool_call),
                input_price: hit
                    .as_ref()
                    .and_then(|h| h.pricing.as_ref())
                    .and_then(|p| p.input),
                output_price: hit
                    .as_ref()
                    .and_then(|h| h.pricing.as_ref())
                    .and_then(|p| p.output),
                family: hit.as_ref().and_then(|h| h.family.clone()),
            }
        })
        .collect();
    crate::llm::recommend::recommend(&candidates)
}

/// 单档探测（设置表单未保存值），失败时端点扫描（§8.2）。`api_key` 空缺
/// 时回退 `profile_id` 指向档位的已存 Key（凭据库双读，编辑既有档位不
/// 重复粘贴 Key）。
#[tauri::command]
pub async fn llm_probe_models(
    kind: String,
    base_url: Option<String>,
    api_key: Option<String>,
    profile_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProbeOutcome, String> {
    let cfg = state.settings.llm_config();
    let stored = profile_id.as_deref().and_then(|id| cfg.profile(id));
    let key = match api_key.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => Some(k.to_string()),
        None => stored.and_then(keyring::resolve),
    };
    let headers = stored.map(|p| p.headers.clone()).unwrap_or_default();
    let outcome = models::list_models_swept(
        &kind,
        base_url.as_deref(),
        key.as_deref(),
        &headers,
        cfg.network_proxy.as_ref(),
    )
    .await;
    let (m, e) = match outcome.result {
        Ok(m) => (m, None),
        Err(e) => (vec![], Some(e)),
    };
    let recommended = recommend_probe_models(&state.catalog, &kind, base_url.as_deref(), &m);
    Ok(ProbeOutcome {
        models: m,
        error: e,
        effective_base_url: outcome.effective_base_url,
        recommended,
    })
}
