//! 网络代理与档位 CRUD（§8.4 / §8.5 / §3.30）：代理保存即热换；档位增删改与排序都经
//! `update_llm_config` 单写口，返回同一份 `project_providers` 投影（永不含秘钥原值，
//! 凭据库在位与明文都算"已配置"）。⚠ `llm_provider_save` 闭包就地做档位装配 / keyring 写 /
//! 清单去重 = 业务进了命令层，已登记 **T25**，本刀纯搬行不夹带下沉。

use tauri::State;

use crate::llm::catalog::CatalogStore;
use crate::llm::keyring;
use crate::settings::{
    new_profile_id, validate_headers, LlmSettingsView, NetworkProxy, ProviderInput, ProviderModel,
    ProviderProfile, ProviderView, PROVIDERS,
};
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

/// 档位增改。`input.id` None = 新建（后端生成 id）；api_key None = 保持
/// 已存值、空串 = 清除。返回全量档位投影（无秘钥字段）。
#[tauri::command]
pub fn llm_provider_save(
    input: ProviderInput,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderView>, String> {
    let label = input.label.trim().to_string();
    if label.is_empty() {
        return Err("档位名称不能为空".into());
    }
    if !PROVIDERS.contains(&input.kind.as_str()) {
        return Err(format!(
            "不支持的 provider: {}（可选 openai / anthropic / ollama）",
            input.kind
        ));
    }
    let base = input
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let cfg = state.settings.update_llm_config(|cfg| {
        let id = match input.id.as_deref().filter(|s| !s.is_empty()) {
            Some(id) => {
                if cfg.profile(id).is_none() {
                    return Err(format!("档位不存在: {id}"));
                }
                id.to_string()
            }
            None => new_profile_id(),
        };
        let mut profile: ProviderProfile = cfg.profile(&id).cloned().unwrap_or(ProviderProfile {
            enabled: true,
            ..Default::default()
        });
        profile.id = id;
        profile.label = label;
        profile.kind = input.kind.clone();
        profile.base_url = base.clone();
        // 自定义请求头（§8.1）：None = 保持已存值；Some = 校验后整表替换
        if let Some(h) = &input.headers {
            validate_headers(h)?;
            profile.headers = h.clone();
        }
        // api_key（§8.4）：None = 保持已存值；空串 = 清除（凭据库随删）；
        // 非空 = 写凭据库 + 换标记（失败回落文件存储，warn 在 keyring 层）
        if let Some(k) = &input.api_key {
            if k.trim().is_empty() {
                keyring::delete(&profile.id);
                profile.api_key = None;
                profile.key_storage = None;
            } else if keyring::store(&profile.id, k) {
                profile.api_key = None;
                profile.key_storage = Some(keyring::KEYRING_MARK.to_string());
            } else {
                profile.api_key = Some(k.clone());
                profile.key_storage = None;
            }
        }
        profile.model = input.model.trim().to_string();
        profile.window_tokens = input.window_tokens;
        profile.enabled = input.enabled.unwrap_or(profile.enabled);
        // models：None = 保持已存清单；Some = 整表替换——trim、按小写去重
        // 保序；原 model 不在清单时改指首项（save 首项 = 该档默认模型）。
        // 目录派生字段在 IPC 边界已被 serde 丢弃，这里只收钉死面。
        if let Some(list) = &input.models {
            let mut seen = std::collections::HashSet::new();
            let mut models_out = Vec::new();
            for m in list {
                let mid = m.id.trim().to_string();
                if mid.is_empty() || !seen.insert(mid.to_lowercase()) {
                    continue;
                }
                models_out.push(ProviderModel {
                    id: mid,
                    alias: m.alias.clone(),
                    context_window: m.context_window,
                    max_output: m.max_output,
                    max_output_source: m.max_output.as_ref().map(|_| "user".to_string()),
                    supports_image: m.supports_image,
                    available_for_subagents: m.available_for_subagents,
                });
            }
            if !models_out.iter().any(|m| m.id == profile.model) {
                profile.model = models_out.first().map(|m| m.id.clone()).unwrap_or_default();
            }
            profile.models = models_out;
        }
        if let Some(slot) = cfg.providers.iter_mut().find(|p| p.id == profile.id) {
            *slot = profile;
        } else {
            cfg.providers.push(profile);
        }
        if cfg.active.is_empty() {
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
