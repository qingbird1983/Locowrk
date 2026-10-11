//! 插件管理命令（DESIGN.md §11.1 / §13.3 / ⑩ 批量）：薄适配层，业务全
//! 在 PluginService。install/update 返回生效版本号，前端据此刷新。

use serde::Serialize;
use tauri::State;

use crate::state::AppState;
use crate::timeutil::now_secs;
use crate::usage::{self, PluginUsageView};
use plugin_service::{InstalledPlugin, MarketplaceItem};

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

/// 启用 / 停用（热插拔）。
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
    let now = now_secs();
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
