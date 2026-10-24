//! 档位装配业务（T25 自命令层下沉）：`llm_provider_save` 闭包里的档位装配 /
//! keyring 写 / 清单去重 / active 指针回落，**只吃 `&mut LlmConfig` 纯改配置**
//! ——命令层只留参数转换、`update_llm_config` 单写口调用与投影。纯函数面
//! （不要 State）让 `llm/tests/profiles.rs` 能直接钉行为，此前命令本体要
//! Tauri `State`，测试打不进去（T6 同款缺口）。

use crate::llm::keyring;
use crate::settings::{
    new_profile_id, validate_headers, LlmConfig, ProviderInput, ProviderModel, ProviderProfile,
    PROVIDERS,
};

/// 档位增改（§8.4）。`input.id` 空 = 新建（后端生成 id）；api_key None =
/// 保持已存值、空串 = 清除（凭据库随删）。行为契约由 `llm/tests/profiles.rs`
/// 逐条钉住；用户可见文案不漂。
pub(crate) fn save_profile(cfg: &mut LlmConfig, input: &ProviderInput) -> Result<(), String> {
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
    profile.base_url = base;
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
}
