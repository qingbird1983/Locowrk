//! LLM 档位与模型清单命令（v2.58，DESIGN §13.1 档位制）：探测（选择器
//! 数据源）+ 档位增删改 + 模型能力目录三件（批 A：状态/刷新/单模型查
//! 询）。命令薄适配——校验在 settings/llm.rs，HTTP 在 llm/models.rs，
//! 目录数据在 llm/catalog.rs，合成在 settings/llm.rs `synthesize_view`。

use serde::Serialize;
use tauri::State;

use crate::llm::catalog::{CatalogStatus, CatalogStore};
use crate::llm::models;
use crate::settings::{
    new_profile_id, LlmSettingsView, ProviderInput, ProviderModel, ProviderProfile, ProviderView,
    PROVIDERS,
};
use crate::state::AppState;

/// 单档模型清单（§3.30 浮层分组行）。`error` = 该档探测失败（如实逐组
/// 报错，不拖垮其它组）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelsView {
    pub id: String,
    pub label: String,
    pub models: Vec<String>,
    pub error: Option<String>,
}

/// 推理梯子落方言口径（批 B）：梯子 = 预设方言的**可发送**档位集
/// （§5.1——目录档位在方言表里没有写法的一律不渲染）；无方言 = 清空
/// （选择器隐藏）。sources.reasoning 标 "preset"（方言来源）。
fn apply_dialect_ladder(v: &mut crate::settings::ProviderModelView, base_url: Option<&str>) {
    let d = crate::llm::dialect_of(base_url);
    if d.ladder().is_empty() {
        v.reasoning_ladder.clear();
        v.sources.reasoning = None;
    } else {
        v.reasoning_ladder = d.ladder().iter().map(|s| s.to_string()).collect();
        v.sources.reasoning = Some("preset".into());
    }
}

/// settings 钉死值 ∪ 目录 = IPC 合成视图（批 A）。settings_get / 档位
/// 命令返回统一走这里——目录数据只进内存不落 settings。
pub fn enrich_with_catalog(catalog: &CatalogStore, llm_view: &mut LlmSettingsView) {
    for p in &mut llm_view.providers {
        p.models = p
            .models
            .iter()
            .map(|m| {
                let hit = catalog.lookup_model(&p.kind, p.base_url.as_deref(), &m.id);
                let mut v = crate::settings::synthesize_view_raw(m, hit.as_ref());
                apply_dialect_ladder(&mut v, p.base_url.as_deref());
                v
            })
            .collect();
    }
}

/// 全部已存档位的模型清单（并行探测，5s 超时逐组收口）。
#[tauri::command]
pub async fn llm_list_models(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderModelsView>, String> {
    let cfg = state.settings.llm_config();
    // 停用档不进选择器清单（§3.30）：与 active 回落同一裁决口径。
    let probes = cfg.providers.iter().filter(|p| p.enabled).map(|p| {
        let id = p.id.clone();
        let label = p.label.clone();
        let kind = p.kind.clone();
        let base = p.base_url.clone();
        let key = p.api_key.clone();
        async move {
            let res = models::list_models(&kind, base.as_deref(), key.as_deref()).await;
            (id, label, res)
        }
    });
    let done = futures::future::join_all(probes).await;
    Ok(done
        .into_iter()
        .map(|(id, label, res)| match res {
            Ok(m) => ProviderModelsView {
                id,
                label,
                models: m,
                error: None,
            },
            Err(e) => ProviderModelsView {
                id,
                label,
                models: vec![],
                error: Some(e),
            },
        })
        .collect())
}

/// 单档探测（设置表单未保存值）。`api_key` 空缺时回退 `profile_id`
/// 指向档位的已存 Key（编辑既有档位不重复粘贴 Key）。
#[tauri::command]
pub async fn llm_probe_models(
    kind: String,
    base_url: Option<String>,
    api_key: Option<String>,
    profile_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let key = match api_key.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => Some(k.to_string()),
        None => profile_id.and_then(|id| {
            state
                .settings
                .llm_config()
                .profile(&id)
                .and_then(|p| p.api_key.clone())
        }),
    };
    models::list_models(&kind, base_url.as_deref(), key.as_deref()).await
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
        // api_key：None = 保持已存值；空串 = 清除
        if let Some(k) = &input.api_key {
            profile.api_key = (!k.is_empty()).then_some(k.clone());
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

/// 档位删除。删的是活动档 = 指针落首档或空。返回全量档位投影。
#[tauri::command]
pub fn llm_provider_delete(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderView>, String> {
    let cfg = state.settings.update_llm_config(|cfg| {
        if cfg.profile(&id).is_none() {
            return Err(format!("档位不存在: {id}"));
        }
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
            api_key_set: p.api_key.is_some(),
            model: p.model.clone(),
            window_tokens: p.window_tokens,
            enabled: p.enabled,
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
