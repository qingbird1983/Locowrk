//! Tauri 命令薄适配层（DESIGN.md §10.3 / §12.1）：只做参数转换与状态
//! 存取，业务逻辑在 runner / approval / kernel，本文件不写一行。

mod llm;
mod memory;
mod plugins;
mod schedule;
mod sessions;

use std::sync::Arc;

pub use llm::*;
pub use memory::*;
pub use plugins::*;
pub use schedule::*;
pub use sessions::*;

use tauri::{AppHandle, State};

use kernel_agent::{CompactConfig, ExposureSnapshot};
use session_store::{SessionEvent, SessionMeta};

use crate::events::{AgentEvent, ApprovalRequestPayload};
use crate::runner::{self, UsagePolicy};
use crate::settings::{
    AppearanceUpdate, ChatUpdate, ContextUpdate, DeveloperUpdate, EmbeddingUpdate, LlmUpdate,
    MemoryUpdate, PluginsUpdate, SettingsView, ShellUpdate, ToolsUpdate,
};
use crate::state::AppState;

/// 聊天命令。轮次内容全部经 `agent://` 事件推送（§12.2），返回值只表达
/// 轮次是否成功收束（返回实际 step 数）。进度条口径从设置取（⑨）：
/// 窗口在 llm 区（S2 迁移），阈值 = 窗口 × 百分比。
#[tauri::command]
pub async fn agent_chat(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    // ⑭：记最近聊过的会话（调度触发的目标解析兜底用）
    state.note_chat(&session_id);
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    let usage = UsagePolicy {
        threshold_tokens: state.settings.effective_threshold_tokens(),
        window_tokens: state.settings.effective_window().unwrap_or(0),
        tools_est_tokens: runner::catalog_estimate(&state.kernel, &session_id).await,
    };
    let steps = runner::run_turn(
        &state.kernel,
        &mut session,
        &session_id,
        message,
        Some(usage),
        state.sink.as_ref(),
        Some(state.usage.as_ref()),
        Some(&state.sessions_dir),
        state.settings.max_turn_steps(),
    )
    .await?;
    // ⑬ 隐式提取（Context 文档 §5.1 ⑤）：开关开启且轮次成功收束后
    // fire-and-forget——本轮消息快照带出会话锁，不阻塞返回值。
    if state.settings.memory().auto_extract {
        let turn = crate::extract::turn_messages(&session.derive_messages());
        crate::extract::spawn_turn_extract(state.extract_deps(), session_id.clone(), turn);
    }
    // v2.57 自动标题：旁车无手动标题时一次 LLM 调用生成（fire-and-forget，
    // 消息快照带出会话锁，不阻塞返回值——⑬ extract 同款）
    crate::title::spawn_auto_title(
        state.app.clone(),
        Arc::clone(&state.settings),
        state.sessions_dir.clone(),
        session_id.clone(),
        session.derive_messages(),
    );
    Ok(steps)
}

/// 撤销最近一次上下文压缩（Context and Memory.md §3.7）：rewind_to(压缩
/// 事件序号) 后 fork 原地换入——前缀游标语义下其后的轮次一并回退。⑲ 起
/// 磁盘镜像随 fork 同步重写（滚动备份 `.undo.bak` = 撤销的物理留痕）。
/// 会话 id 不变，前端无需重新绑定。返回回退点（一般无需使用）。
#[tauri::command]
pub async fn context_undo_compact(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    let seq = session.events()[..session.cursor()]
        .iter()
        .rposition(|e| matches!(e, SessionEvent::ContextCompacted { .. }))
        .ok_or_else(|| "没有可撤销的压缩记录".to_string())?;
    session.rewind_to(seq);
    *session = session.fork(seq);
    crate::session_log::rewrite_after_undo(&state.sessions_dir, &session_id, &session);
    Ok(seq)
}

// ---- 会话持久化命令（⑲）：列表与全量投影。历史恢复不走事件通道
// （agent:// 只服务当轮增量，§4.2）。 ----

/// 会话列表（`sessions/` 目录按 mtime 降序；空会话无文件不出现——
/// 首条消息发出前不创建任何后端状态）。
#[tauri::command]
pub async fn session_list(state: State<'_, AppState>) -> Result<Vec<SessionMeta>, String> {
    Ok(crate::session_log::list(&state.sessions_dir))
}

/// 会话全量投影：整个日志按 runner 同一映射转 UI 事件序列，前端重放
/// 恢复气泡/工具卡/系统通知。会话不存在 = 空表（新会话走正常聊天流）。
#[tauri::command]
pub async fn session_history(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let Some(lock) = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned()
    else {
        return Ok(Vec::new());
    };
    let session = lock.lock().await;
    Ok(runner::project_events(session.events(), &session_id))
}

// 审批决策（§12.1）：回填 `agent://approval-request` 对应的请求。
/// `scope = Some("session")` 且放行 = §3.17「允许本次对话」（豁免集记在
/// ApprovalHub，本会话后续同工具不再问）。与会话锁无交集，决策卡在屏时
/// 聊天通道仍可安全阻塞等待。
#[tauri::command]
pub async fn agent_approve(
    request_id: String,
    allowed: bool,
    reason: Option<String>,
    scope: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .approval
        .decide(&request_id, allowed, reason, scope.as_deref())
}

/// 未决审批列表（§13.1 审批中心）：前端刷新/重连时补拉。
/// （Tauri 约定：async 命令含引用入参必须返回 Result，这里恒为 Ok。）
#[tauri::command]
pub async fn list_pending_approvals(
    state: State<'_, AppState>,
) -> Result<Vec<ApprovalRequestPayload>, String> {
    Ok(state.approval.pending())
}

// ---- 设置命令（DESIGN.md §13.1 / §13.5）：薄适配层，校验与落盘全在
// SettingsStore。返回投影永不含 API Key 原值，只有 api_key_set。 ----

/// 读设置（LLM 配置投影 + 数据根位置 + 语言偏好）。
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<SettingsView, String> {
    Ok(state.settings.view())
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
    shell: Option<ShellUpdate>,
    plugins: Option<PluginsUpdate>,
    chat: Option<ChatUpdate>,
    developer: Option<DeveloperUpdate>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsView, String> {
    let plugins_section_touched = plugins.is_some();
    let tools_section_touched = tools.is_some();
    let view = state.settings.update(
        locale.clone(),
        appearance,
        shortcut.clone(),
        llm,
        embedding,
        context,
        memory,
        tools,
        shell,
        plugins,
        chat,
        developer,
    )?;
    if let Some(sc) = shortcut.filter(|s| !s.trim().is_empty()) {
        crate::tray::apply_shortcut(&app, sc.trim())?;
    }
    // 语言改动 → 托盘菜单即时换文案（失败仅日志，不影响保存结果）
    if locale.is_some() {
        crate::tray::refresh_menu(&app);
    }
    // ⑨：压缩配置热更新（阈值/策略/窗口换入共享锁）
    let cfg: CompactConfig = state.settings.compact_config();
    if let Ok(mut guard) = state.compact_cfg.write() {
        *guard = cfg;
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
    // ㉘·B：新会话权限档缺省热更新（tools 区 defaultPermissionMode）
    state
        .permission_modes
        .set_default(state.settings.default_permission_mode());
    // ⑱：工作区目录热更新（fsutil 每次工具调用读 env，重注入即生效）
    if tools_section_touched {
        crate::state::apply_fs_roots_env(&state.settings);
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
    Ok(view)
}

// ---- 模型窗口探测（S2，清单 §5）：Ollama /api/show 自动提取，读不到
// 由 UI 提示手动选择。业务在 llm/probe.rs。 ----

/// 探测 Ollama 模型的上下文窗口（token）。base_url 空 = 默认端点。
#[tauri::command]
pub async fn llm_probe_window(base_url: Option<String>, model: String) -> Result<u64, String> {
    if model.trim().is_empty() {
        return Err("模型名为空，无法探测".into());
    }
    crate::llm::probe::probe_context_window(base_url.as_deref(), model.trim()).await
}

/// 探测 embedding 端点（⑬）：按给定端点/模型嵌一条测试文本，返回向量
/// 维度（UI 显示"维度 768"之类，失败报 Err 由 UI 提示）。与保存解耦——
/// 这里只探测不落盘。
#[tauri::command]
pub async fn llm_probe_embedding(
    base_url: Option<String>,
    model: String,
    api_key: Option<String>,
) -> Result<usize, String> {
    if model.trim().is_empty() {
        return Err("embedding 模型名为空，无法探测".into());
    }
    let client = crate::llm::EmbedClient {
        base_url: base_url
            .unwrap_or_else(|| "http://localhost:11434".into())
            .trim_end_matches('/')
            .to_string(),
        model: model.trim().to_string(),
        api_key: api_key.filter(|k| !k.is_empty()),
    };
    client
        .embed(&["locowrk probe".to_string()])
        .await
        .and_then(|vecs| {
            vecs.first()
                .map(|v| v.len())
                .ok_or_else(|| "embedding 响应为空".into())
        })
}

// ---- 工具暴露调试命令（⑫，exposure 文档 §6.3/§9.6）：开发者选项读最近
// 一次截断快照。快照由 ToolExposure 在每个 agent/request 上刷新，前端只在
// 打开面板时拉一次（§8.8"用户来找"的信息，不推送不轮询）。 ----

/// 最近一次请求的工具暴露快照（总数/暴露数/always/会话热/全局热/节省估算）。
#[tauri::command]
pub async fn tool_exposure_debug(state: State<'_, AppState>) -> Result<ExposureSnapshot, String> {
    Ok(state.exposure.snapshot())
}

// ---- 原生目录选择（⑱，UI shell 文档 §7 规则 8）：tauri-plugin-dialog 仅
// Rust 侧调用（同 notification 先例：无 JS guest、无需 capability）。async
// 命令跑在异步运行时线程而非主线程，blocking API 安全。取消/关窗 = None，
// 是否采纳由调用分区决定。 ----

/// 打开 OS 原生目录选择对话框，返回所选目录绝对路径（取消返回 None）。
#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_folder();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string()))
}

/// 打开 OS 原生存储对话框（v2.49 会话导出用，§七 规则 8 同族：仅 Rust
/// 侧调用）：`default_name` 建议文件名（空 = 不预设），返回所选绝对路径
/// （取消返回 None）。
#[tauri::command]
pub async fn pick_save_path(
    default_name: String,
    app: AppHandle,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file();
    let name = default_name.trim();
    if !name.is_empty() {
        builder = builder.set_file_name(name);
    }
    let picked = builder.blocking_save_file();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string()))
}

/// 原生多选文件对话框（§3.11 v3.9 composer「＋添加文件或图片」）：仅
/// Rust 侧调用（§七 规则 8 同族），返回所选绝对路径数组（取消 = 空表）。
/// 路径由前端以 `@路径` 引用写进输入框——附件随消息上传的消息通路属
/// ㉛ 排期，当前模型经 fs 工具按路径读取（v1 语义，宁缺勿假）。
#[tauri::command]
pub async fn pick_files(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_files();
    let mut out = Vec::new();
    for p in picked.into_iter().flatten() {
        if let Ok(path) = p.into_path() {
            out.push(path.display().to_string());
        }
    }
    Ok(out)
}

// ---- 应用内通知（v2.51 §3.13）：读 / 全读 / 单读 / 清空，薄适配层 ----

#[tauri::command]
pub async fn notification_list(
    state: State<'_, AppState>,
) -> Result<Vec<crate::notifications::NotificationItem>, String> {
    Ok(state.notifications.list())
}

#[tauri::command]
pub async fn notification_read_all(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.notifications.mark_all_read())
}

/// 单条置读（行点击跳会话时），返回是否命中。
#[tauri::command]
pub async fn notification_read(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.notifications.mark_read(&id))
}

#[tauri::command]
pub async fn notification_clear(state: State<'_, AppState>) -> Result<(), String> {
    state.notifications.clear();
    Ok(())
}
