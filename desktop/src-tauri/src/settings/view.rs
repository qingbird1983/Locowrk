//! IPC 投影组装（M2 起自 settings.rs 拆出——单文件 400 行纪律）：
//! `SettingsView` 的字段映射是唯一职责，无业务逻辑。API Key 只落盘不出
//! IPC（各投影只有 `api_key_set` 布尔）。

use std::path::Path;

use super::{
    synthesize_view, AppSettings, AppearanceView, ChatView, ContextSettingsView, DeveloperView,
    EmbeddingView, LlmSettingsView, MemorySettingsView, MultimodalView, PluginsView, ProviderView,
    SettingsView, ToolsSettingsView,
};

pub(crate) fn project(s: &AppSettings, data_dir: &Path) -> SettingsView {
    // S2：llm 平面投影（窗口等档位原样；v2.137 起 context 投影不再吃窗口）
    let flat = s.llm.flat();
    SettingsView {
        locale: s.locale.clone(),
        appearance: AppearanceView {
            theme: s.appearance.theme.clone(),
            ui_font_size: s.appearance.ui_font_size,
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
                    // 明文或凭据库在位都算已配置（批 C §8.4）
                    api_key_set: p.has_key(),
                    model: p.model.clone(),
                    window_tokens: p.window_tokens,
                    enabled: p.enabled,
                    headers: p.headers.clone(),
                    // 无目录铺底（钉死面原样、能力未知）；命令层 enrich
                    // 挂 models.dev 数据（批 A）——本函数保持纯投影。
                    models: p.models.iter().map(|m| synthesize_view(m, None)).collect(),
                })
                .collect(),
            provider: flat.provider,
            model: flat.model,
            base_url: flat.base_url,
            api_key_set: flat.api_key.is_some(),
            window_tokens: flat.window_tokens,
            network_proxy: s.llm.network_proxy.clone(),
            subagent: s.llm.subagent.clone(),
        },
        embedding: EmbeddingView {
            base_url: s.embedding.base_url.clone(),
            model: s.embedding.model.clone(),
            api_key_set: s.embedding.api_key.is_some(),
        },
        context: ContextSettingsView {
            threshold_tokens: s.context.threshold_tokens,
            usage_bar: s.context.usage_bar,
            tool_result_budget_kib: s.context.tool_result_budget_kib,
            micro_enabled: s.context.micro_enabled,
            compact_enabled: s.context.compact_enabled,
            compact_target_pct: s.context.compact_target_pct,
            compact_keep_tokens: s.context.compact_keep_tokens,
            compact_model: s.context.compact_model.clone(),
            compact_instructions: s.context.compact_instructions.clone(),
            micro_keep_groups: s.context.micro_keep_groups,
            slide_keep_runs: s.context.slide_keep_runs,
        },
        memory: MemorySettingsView {
            enabled: s.memory.enabled,
            top_k: s.memory.top_k,
            organize_pacing: s.memory.organize_pacing.clone(),
            organize_chunk_tokens: s.memory.organize_chunk_tokens,
            memory_manager_model: s.memory.memory_manager_model.clone(),
            memory_manager_provider: s.memory.memory_manager_provider.clone(),
            tail_strategy: s.memory.tail_strategy.clone(),
            tail_window_turns: s.memory.tail_window_turns,
            collapse_enabled: s.memory.collapse_enabled,
            collapse_threshold_pct: s.memory.collapse_threshold_pct,
            files: s.memory.files,
            repo_context: s.memory.repo_context,
            global_layer: s.memory.global_layer,
            project_layer: s.memory.project_layer,
            instruction_files: s.memory.instruction_files,
            instruction_global_kib: s.memory.instruction_global_kib,
            instruction_project_kib: s.memory.instruction_project_kib,
            fact_cards: s.memory.fact_cards,
            fact_card_max: s.memory.fact_card_max,
        },
        tools: ToolsSettingsView {
            enabled: s.tools.enabled,
            max_per_request: s.tools.max_per_request,
            call_timeout_secs: s.tools.call_timeout_secs,
            fs_roots: s.tools.fs_roots.clone(),
            max_turn_steps: s.tools.max_turn_steps,
            approval_timeout_secs: s.tools.approval_timeout_secs,
            ask_timeout_secs: s.tools.ask_timeout_secs,
            default_permission_mode: s.tools.default_permission_mode.clone(),
            pin_session: s.tools.pin_session,
        },
        plugins: PluginsView {
            auto_disable_idle_days: s.plugins.auto_disable_idle_days,
        },
        chat: ChatView {
            content_max_width: s.chat.content_max_width,
            collapse_tool_process: s.chat.collapse_tool_process,
            enter_to_send: s.chat.enter_to_send,
            usage_readout: s.chat.usage_readout.clone(),
            auto_title: s.chat.auto_title,
            cache_stats: s.chat.cache_stats,
            fold_long_messages: s.chat.fold_long_messages,
            fold_lines: s.chat.fold_lines,
        },
        developer: DeveloperView {
            unlocked: s.developer.unlocked,
            allow_delete_running_session: s.developer.allow_delete_running_session,
            allow_switch_running_session: s.developer.allow_switch_running_session,
            cache_hit_alert: s.developer.cache_hit_alert,
            cache_diagnostics: s.developer.cache_diagnostics,
            cache_hit_alert_percent: s.developer.cache_hit_alert_percent,
            organize_max_rerun: s.developer.organize_max_rerun,
            organize_rebound_k: s.developer.organize_rebound_k,
            organize_rebound_m: s.developer.organize_rebound_m,
            memory_manager_timeout_secs: s.developer.memory_manager_timeout_secs,
            memory_manager_max_retries: s.developer.memory_manager_max_retries,
            rule_chunk_tokens: s.developer.rule_chunk_tokens,
            organize_max_release: s.developer.organize_max_release,
            organize_release_watermark_pct: s.developer.organize_release_watermark_pct,
            organize_summary_chapters: s.developer.organize_summary_chapters,
            organize_summary_attempts: s.developer.organize_summary_attempts,
        },
        thinking: super::ThinkingView {
            language: s.thinking.language.clone(),
        },
        multimodal: MultimodalView {
            max_dimension: s.multimodal.max_dimension,
            max_size_mb: s.multimodal.max_size_mb,
            image_formats: s.multimodal.image_formats.clone(),
            file_formats: s.multimodal.file_formats.clone(),
            auto_gc: s.multimodal.auto_gc,
            gc_threshold_days: s.multimodal.gc_threshold_days,
        },
    }
}
