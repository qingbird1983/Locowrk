//! 网络代理与档位 CRUD（§8.4 / §8.5 / §3.30）：代理保存即热换；档位增删改与排序都经
//! `update_llm_config` 单写口，返回同一份 `project_providers` 投影（永不含秘钥原值，
//! 凭据库在位与明文都算"已配置"）。`llm_provider_save` 的装配业务已下沉
//! `llm::profiles::save_profile`（T25，2026-10-24）——本片只剩薄适配：参数转换 +
//! 单写口调用 + 投影。

use tauri::State;

use crate::llm::catalog::CatalogStore;
use crate::llm::keyring;
use crate::settings::{LlmSettingsView, NetworkProxy, ProviderInput, ProviderView};
use crate::state::AppState;

use super::views::{apply_dialect_ladder, enrich_with_catalog};

/// 网络代理设置（§8.5）：保存即热换（客户端逐请求新建，下一次请求生效）。
/// mode = system 时存 None（settings 零噪声）。
#[tauri::command]
pub fn llm_set_proxy(
    mode: String,
    url: Option<String>,
    state: State<'_, AppState>,
) -> Result<LlmSettingsView, String> {
    if mode == "system" {
        state.settings.update_llm_config(|cfg| {
            cfg.network_proxy = None;
            Ok(())
        })?;
    } else {
        let np = NetworkProxy { mode, url }.normalized();
        np.validate()?;
        state.settings.update_llm_config(|cfg| {
            cfg.network_proxy = Some(np.clone());
            Ok(())
        })?;
    }
    // 返回与 settings_get 同形的 llm 区视图（目录合成随行）
    let mut view = state.settings.view().llm;
    enrich_with_catalog(&state.catalog, &mut view);
    Ok(view)
}

/// 档位增改：薄适配（T25）——校验、装配、keyring 写与清单去重都在
/// `llm::profiles::save_profile`，这里只接参数、过单写口、回投影。
#[tauri::command]
pub fn llm_provider_save(
    input: ProviderInput,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderView>, String> {
    let cfg = state
        .settings
        .update_llm_config(|cfg| crate::llm::profiles::save_profile(cfg, &input))?;
    Ok(project_providers(&state.catalog, &cfg))
}

/// 档位删除。删的是活动档 = 指针落首档或空。凭据库条目随删（best-
/// effort）。返回全量档位投影。
#[tauri::command]
pub fn llm_provider_delete(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderView>, String> {
    let cfg = state.settings.update_llm_config(|cfg| {
        if cfg.profile(&id).is_none() {
            return Err(format!("档位不存在: {id}"));
        }
        keyring::delete(&id);
        cfg.providers.retain(|p| p.id != id);
        if cfg.active == id {
            cfg.active = cfg
                .providers
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_default();
        }
        Ok(())
    })?;
    Ok(project_providers(&state.catalog, &cfg))
}

/// 档位排序（§3.30 服务列表拖拽落盘）：按传入 id 顺序整表重排。
#[tauri::command]
pub fn llm_provider_reorder(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderView>, String> {
    let cfg = state.settings.update_llm_config(|cfg| {
        if cfg.providers.len() != ids.len() {
            return Err("排序清单与档位表长度不一致".into());
        }
        let mut next = Vec::with_capacity(ids.len());
        for id in &ids {
            let p = cfg.profile(id).ok_or_else(|| format!("档位不存在: {id}"))?;
            next.push(p.clone());
        }
        cfg.providers = next;
        Ok(())
    })?;
    Ok(project_providers(&state.catalog, &cfg))
}

fn project_providers(
    catalog: &CatalogStore,
    cfg: &crate::settings::LlmConfig,
) -> Vec<ProviderView> {
    cfg.providers
        .iter()
        .map(|p| ProviderView {
            id: p.id.clone(),
            label: p.label.clone(),
            kind: p.kind.clone(),
            base_url: p.base_url.clone(),
            // 明文或凭据库在位都算已配置（批 C §8.4）
            api_key_set: p.has_key(),
            model: p.model.clone(),
            window_tokens: p.window_tokens,
            enabled: p.enabled,
            headers: p.headers.clone(),
            models: p
                .models
                .iter()
                .map(|m| {
                    let hit = catalog.lookup_model(&p.kind, p.base_url.as_deref(), &m.id);
                    let mut v = crate::settings::synthesize_view(m, hit.as_ref());
                    apply_dialect_ladder(&mut v, p.base_url.as_deref());
                    v
                })
                .collect(),
        })
        .collect()
}
