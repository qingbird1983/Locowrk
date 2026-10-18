//! 设置读写（DESIGN §13.1 / §13.5）：参数 = settings.json 各分区一一对应（§3.2），
//! 校验与落盘全在 SettingsStore；本文件只负责保存后的热更新广播（各配置换入共享锁、
//! 托盘文案、env 重注入、自动停用巡检）。返回投影永不含 API Key 原值。

use std::sync::Arc;

use tauri::{AppHandle, State};

use super::llm::enrich_with_catalog;
use crate::settings::{
    AppearanceUpdate, ChatUpdate, ContextUpdate, DeveloperUpdate, EmbeddingUpdate, LlmUpdate,
    MemoryUpdate, MultimodalUpdate, PluginsUpdate, SettingsView, ThinkingUpdate, ToolsUpdate,
};
use crate::state::AppState;

// ---- 设置命令（DESIGN.md §13.1 / §13.5）：薄适配层，校验与落盘全在
// SettingsStore。返回投影永不含 API Key 原值，只有 api_key_set。 ----

/// 读设置（LLM 配置投影 + 数据根位置 + 语言偏好）。批 A：llm 档位模型
/// 清单挂目录合成（settings 钉死值 ∪ models.dev，目录只进内存）。
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<SettingsView, String> {
    let mut view = state.settings.view();
    enrich_with_catalog(&state.catalog, &mut view.llm);
    Ok(view)
}

/// 部分更新设置：locale、外观区（⑰ 明暗主题）、快捷键、LLM 字段（S2 起
/// 含 windowTokens——窗口归属模型配置；传 null = 清除，回落保守保护）、
/// ⑬ embedding 区（端点/模型/Key）、上下文管理区（⑨，仅阈值+策略）、
/// 记忆注入区（⑪，⑬ 起含 autoExtract）、工具暴露区（⑫，S2 起含单次调用
/// 超时）、shell 治理区（S1）与 ⑬ 插件管理区（自动停用天数）独立可选，
/// 缺省字段保持不变；base_url / api_key 传空串 = 清除。改键成功后即时
/// 重注册全局快捷键；注册失败返回 Err（新键已落盘，换键重试即可）。上下文
/// / 记忆 / 工具暴露 / 超时 / embedding 改动成功后换入活配置随即生效；
/// shell 解锁开关经插件加载前的 env 注入，重启后生效；插件自动停用天数
/// > 0 时保存即巡检一遍（0 = 关闭，不自动恢复任何已停用插件）。
// 参数 = settings.json 各分区一一对应（§3.2）；Tauri command 的扁平
// IPC 形状不收拢成结构体。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn settings_set(
    locale: Option<String>,
    appearance: Option<AppearanceUpdate>,
    shortcut: Option<String>,
    llm: Option<LlmUpdate>,
    embedding: Option<EmbeddingUpdate>,
    context: Option<ContextUpdate>,
    memory: Option<MemoryUpdate>,
    tools: Option<ToolsUpdate>,
    plugins: Option<PluginsUpdate>,
    chat: Option<ChatUpdate>,
    developer: Option<DeveloperUpdate>,
    thinking: Option<ThinkingUpdate>,
    multimodal: Option<MultimodalUpdate>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsView, String> {
    let plugins_section_touched = plugins.is_some();
    let tools_section_touched = tools.is_some();
    let memory_section_touched = memory.is_some();
    let view = state.settings.update(
        locale.clone(),
        appearance,
        shortcut.clone(),
        llm,
        embedding,
        context,
        memory,
        tools,
        plugins,
        chat,
        developer,
        thinking,
        multimodal,
    )?;
    if let Some(sc) = shortcut.filter(|s| !s.trim().is_empty()) {
        crate::tray::apply_shortcut(&app, sc.trim())?;
    }
    // 语言改动 → 托盘菜单即时换文案（失败仅日志，不影响保存结果）
    if locale.is_some() {
        crate::tray::refresh_menu(&app);
    }
    // ⑪：记忆注入配置热更新（开关/topK 换入共享锁）
    let mcfg = state.settings.memory_inject_config();
    if let Ok(mut guard) = state.memory_cfg.write() {
        *guard = mcfg;
    }
    // ⑫：工具暴露配置热更新（开关/每请求最大工具数换入共享锁）
    let ecfg = state.settings.exposure_config();
    if let Ok(mut guard) = state.exposure_cfg.write() {
        *guard = ecfg;
    }
    // S2：宿主超时时长热更新（settings tools 区 callTimeoutSecs）
    if let Ok(mut guard) = state.timeout_cfg.write() {
        *guard = state.settings.call_timeout();
    }
    // v2.49：审批守夜超时热更新（tools 区 approvalTimeoutSecs，0 = 不限）
    state
        .approval
        .set_timeout_secs(state.settings.approval_timeout_secs());
    // v4.11 §3.39①：ask_user 守夜时长热更新（tools 区 askTimeoutSecs，0 = 不限）
    state
        .ask
        .set_timeout_secs(state.settings.ask_timeout_secs());
    // ㉘·B：新会话权限档缺省热更新（tools 区 defaultPermissionMode）
    state
        .permission_modes
        .set_default(state.settings.default_permission_mode());
    // ⑱：工作区目录热更新（fsutil 每次工具调用读 env，重注入即生效）
    if tools_section_touched {
        crate::data_root::apply_fs_roots_env(&state.settings);
    }
    // F1：记忆文件化总开关热更新（插件侧 memlayout 每次写前读 env，保存
    // 即生效；宿主侧 MemoryStore/导出走 files_on 闭包本就实时）。
    if memory_section_touched {
        std::env::set_var(
            "LOCOWRK_MEMORY_FILES",
            if state.settings.memory().files {
                "1"
            } else {
                "0"
            },
        );
    }
    // ⑬：embedding 客户端热重建（检索信号/search_tools/经历/提取共用）
    state.reload_embed();
    // ⑬：插件自动停用巡检（缺省关；仅在天数 > 0 时跑，0 不复活已停用项）
    if plugins_section_touched {
        let days = state.settings.plugins().auto_disable_idle_days;
        if days > 0 {
            let plugins_svc = Arc::clone(&state.plugins);
            let usage_rec = Arc::clone(&state.usage);
            tokio::spawn(async move {
                crate::autodisable::auto_disable_idle(&plugins_svc, &usage_rec, days).await;
            });
        }
    }
    let mut view = view;
    enrich_with_catalog(&state.catalog, &mut view.llm);
    Ok(view)
}
