//! IPC 投影组装（M2 起自 settings.rs 拆出——单文件 400 行纪律）：
//! `SettingsView` 的字段映射是唯一职责，无业务逻辑。API Key 只落盘不出
//! IPC（各投影只有 `api_key_set` 布尔）。

use std::path::Path;

use super::{
    AppSettings, AppearanceView, ChatView, ContextSettingsView, DeveloperView, EmbeddingView,
    LlmSettingsView, MemorySettingsView, PluginsView, ProviderView, SettingsView,
    ShellSettingsView, ToolsSettingsView, FALLBACK_WINDOW_TOKENS,
};

pub(crate) fn project(s: &AppSettings, data_dir: &Path) -> SettingsView {
    // S2：窗口以 llm 活动档为准（context 旧字段只在迁移前的旧文件里出现）
    let flat = s.llm.flat();
    let window = flat.window_tokens.or(s.context.window_tokens);
    SettingsView {
        locale: s.locale.clone(),
        appearance: AppearanceView {
            theme: s.appearance.theme.clone(),
        },
        shortcut: s.shortcut.clone(),
        data_dir: data_dir.display().to_string(),
        llm: LlmSettingsView {
            active: s.llm.active.clone(),
            providers: s
                .llm
                .providers
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
                    models: p.models.clone(),
                })
                .collect(),
            provider: flat.provider,
            model: flat.model,
            base_url: flat.base_url,
            api_key_set: flat.api_key.is_some(),
            window_tokens: flat.window_tokens,
        },
        embedding: EmbeddingView {
            base_url: s.embedding.base_url.clone(),
            model: s.embedding.model.clone(),
            api_key_set: s.embedding.api_key.is_some(),
        },
        context: ContextSettingsView {
            threshold_pct: s.context.threshold_pct,
            strategy: s.context.strategy,
            stability_rounds: s.context.stability_rounds,
            threshold_tokens: s
                .context
                .effective_threshold(window.unwrap_or(FALLBACK_WINDOW_TOKENS)),
            suggested_pct: s.context.suggested_pct(window),
        },
        memory: MemorySettingsView {
            enabled: s.memory.enabled,
            top_k: s.memory.top_k,
            auto_extract: s.memory.auto_extract,
            organize_pacing: s.memory.organize_pacing.clone(),
            organize_chunk_tokens: s.memory.organize_chunk_tokens,
            recorder_model: s.memory.recorder_model.clone(),
            recorder_provider: s.memory.recorder_provider.clone(),
        },
        tools: ToolsSettingsView {
            enabled: s.tools.enabled,
            max_per_request: s.tools.max_per_request,
            call_timeout_secs: s.tools.call_timeout_secs,
            fs_roots: s.tools.fs_roots.clone(),
            max_turn_steps: s.tools.max_turn_steps,
            approval_timeout_secs: s.tools.approval_timeout_secs,
            default_permission_mode: s.tools.default_permission_mode.clone(),
            pin_session: s.tools.pin_session,
        },
        shell: ShellSettingsView {
            unlock_risky: s.shell.unlock_risky,
        },
        plugins: PluginsView {
            auto_disable_idle_days: s.plugins.auto_disable_idle_days,
        },
        chat: ChatView {
            content_max_width: s.chat.content_max_width,
            expand_tool_process: s.chat.expand_tool_process,
            enter_to_send: s.chat.enter_to_send,
            usage_readout: s.chat.usage_readout.clone(),
            auto_title: s.chat.auto_title,
            cache_stats: s.chat.cache_stats,
            cache_hit_price: s.chat.cache_hit_price,
            cache_miss_price: s.chat.cache_miss_price,
        },
        developer: DeveloperView {
            unlocked: s.developer.unlocked,
            allow_delete_running_session: s.developer.allow_delete_running_session,
            allow_switch_running_session: s.developer.allow_switch_running_session,
            cache_hit_alert: s.developer.cache_hit_alert,
            cache_hit_alert_percent: s.developer.cache_hit_alert_percent,
            organize_rebound_k: s.developer.organize_rebound_k,
            organize_rebound_m: s.developer.organize_rebound_m,
            recorder_timeout_secs: s.developer.recorder_timeout_secs,
            recorder_max_retries: s.developer.recorder_max_retries,
            rule_chunk_tokens: s.developer.rule_chunk_tokens,
        },
    }
}
