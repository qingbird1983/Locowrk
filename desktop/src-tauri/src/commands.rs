//! Tauri 命令薄适配层（DESIGN.md §10.3 / §12.1）：只做参数转换与状态
//! 存取，业务逻辑在 runner / approval / kernel，本文件不写一行。

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, State};

use kernel_agent::CompactConfig;
use session_store::SessionEvent;

use crate::events::ApprovalRequestPayload;
use crate::runner::{self, UsagePolicy};
use crate::settings::{ContextUpdate, LlmUpdate, SettingsView};
use crate::state::AppState;
use crate::usage::{self, PluginUsageView};
use plugin_service::{InstalledPlugin, MarketplaceItem};

/// 聊天命令。轮次内容全部经 `agent://` 事件推送（§12.2），返回值只表达
/// 轮次是否成功收束（返回实际 step 数）。进度条口径从设置取（⑨）。
#[tauri::command]
pub async fn agent_chat(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    let context = state.settings.context();
    let usage = UsagePolicy {
        threshold_tokens: context.effective_threshold(),
        window_tokens: context.window_tokens.unwrap_or(0),
    };
    runner::run_turn(
        &state.kernel,
        &mut session,
        &session_id,
        message,
        Some(usage),
        state.sink.as_ref(),
        Some(state.usage.as_ref()),
    )
    .await
}

/// 撤销最近一次上下文压缩（Context and Memory.md §3.7）：rewind_to(压缩
/// 事件序号) 后 fork 原地换入——前缀游标语义下其后的轮次一并回退，日志
/// 只追加不删除的纪律由"日志是内存副本"兜底（落盘持久化在收尾 backlog）。
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
    Ok(seq)
}

/// 审批决策（§12.1）：回填 `agent://approval-request` 对应的请求。
/// 与会话锁无交集，审批弹窗打开时聊天通道仍可安全阻塞等待。
#[tauri::command]
pub async fn agent_approve(
    request_id: String,
    allowed: bool,
    reason: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.approval.decide(&request_id, allowed, reason)
}

/// 未决审批列表（§13.1 审批中心）：前端刷新/重连时补拉。
/// （Tauri 约定：async 命令含引用入参必须返回 Result，这里恒为 Ok。）
#[tauri::command]
pub async fn list_pending_approvals(
    state: State<'_, AppState>,
) -> Result<Vec<ApprovalRequestPayload>, String> {
    Ok(state.approval.pending())
}

// ---- 插件管理命令（DESIGN.md §11.1 / §13.3）：薄适配层，业务全在
// PluginService。install/update 返回生效版本号，前端据此刷新。 ----

/// 已安装列表（lockfile 投影 + 注册表运行态合并）。
#[tauri::command]
pub async fn plugin_list(state: State<'_, AppState>) -> Result<Vec<InstalledPlugin>, String> {
    Ok(state.plugins.list_installed())
}

/// 市场目录（发现页）。市场目录缺失时为空列表，不是错误。
#[tauri::command]
pub async fn plugin_registry_list(
    state: State<'_, AppState>,
) -> Result<Vec<MarketplaceItem>, String> {
    state.plugins.catalog()
}

/// 从市场安装；`version` 缺省取市场 latest。返回实际安装的版本。
#[tauri::command]
pub async fn plugin_install(
    id: String,
    version: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    state
        .plugins
        .install(&id, version.as_deref())
        .await
        .map(|v| v.to_string())
}

/// 启用 / 禁用（热插拔）。
#[tauri::command]
pub async fn plugin_set_enabled(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.plugins.set_enabled(&id, enabled).await
}

/// 卸载：逻辑卸载（graveyard）→ 撤 Effect → 删目录（Windows 文件锁删
/// 不掉则 tombstone，下次启动清理）。
#[tauri::command]
pub async fn plugin_uninstall(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.plugins.uninstall(&id).await
}

/// 回退到指定版本（切 active 指针，旧版本永远保留）。返回回退到的版本。
#[tauri::command]
pub async fn plugin_rollback(
    id: String,
    version: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    state.plugins.rollback(&id, &version).await?;
    Ok(version)
}

/// 更新到市场 latest（失败自动回滚旧指针）。返回新版本。
#[tauri::command]
pub async fn plugin_update(id: String, state: State<'_, AppState>) -> Result<String, String> {
    state.plugins.update(&id).await.map(|v| v.to_string())
}

/// 批量操作结果（⑩⑥，exposure 文档 §10.2）：逐条成败，部分失败不回滚。
#[derive(Debug, Default, Serialize)]
pub struct BatchFailure {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Default, Serialize)]
pub struct BatchResult {
    pub ok: Vec<String>,
    pub failed: Vec<BatchFailure>,
}

/// 带使用统计的已安装列表（⑩⑤）：闲置分级与筛选档位全在后端裁决
/// （exposure 文档 §10.1——前端不做时间计算）。
#[tauri::command]
pub async fn plugin_list_with_usage(
    state: State<'_, AppState>,
) -> Result<Vec<PluginUsageView>, String> {
    let installed = state.plugins.list_installed();
    let times = state.usage.plugin_times();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(usage::attach(&installed, now, &times))
}

/// 批量启用/停用（⑩⑥）。逐条执行，失败项列进 failed，成功项照常生效。
#[tauri::command]
pub async fn plugin_batch_set_enabled(
    ids: Vec<String>,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<BatchResult, String> {
    let mut result = BatchResult::default();
    for id in ids {
        match state.plugins.set_enabled(&id, enabled).await {
            Ok(()) => result.ok.push(id),
            Err(error) => result.failed.push(BatchFailure { id, error }),
        }
    }
    Ok(result)
}

/// 批量卸载（⑩⑥）。逐条执行（graveyard → 撤 Effect → 删目录）。
#[tauri::command]
pub async fn plugin_batch_uninstall(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<BatchResult, String> {
    let mut result = BatchResult::default();
    for id in ids {
        match state.plugins.uninstall(&id).await {
            Ok(()) => result.ok.push(id),
            Err(error) => result.failed.push(BatchFailure { id, error }),
        }
    }
    Ok(result)
}

// ---- 设置命令（DESIGN.md §13.1 / §13.5）：薄适配层，校验与落盘全在
// SettingsStore。返回投影永不含 API Key 原值，只有 api_key_set。 ----

/// 读设置（LLM 配置投影 + 数据根位置 + 语言偏好）。
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<SettingsView, String> {
    Ok(state.settings.view())
}

/// 部分更新设置：locale、快捷键、LLM 字段与上下文管理区（⑨）独立可选，
/// 缺省字段保持不变；base_url / api_key 传空串 = 清除；context 的
/// windowTokens 传 null = 清除窗口（回落保守保护）。改键成功后即时
/// 重注册全局快捷键；注册失败返回 Err（新键已落盘，换键重试即可）。
/// 上下文改动成功后换入 ctx-compact 活配置——压缩行为随即生效。
#[tauri::command]
pub async fn settings_set(
    locale: Option<String>,
    shortcut: Option<String>,
    llm: Option<LlmUpdate>,
    context: Option<ContextUpdate>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsView, String> {
    let view = state
        .settings
        .update(locale.clone(), shortcut.clone(), llm, context)?;
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
    Ok(view)
}
