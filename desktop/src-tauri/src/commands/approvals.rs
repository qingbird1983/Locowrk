//! 审批决策与项目粘性授权规则（§12.1 / §13.1 / v2.145 设置管理行）。
//! 决策本体在 ApprovalHub、规则本体在 PermissionRulesStore，本文件只做薄适配。

use tauri::State;

use crate::events::ApprovalRequestPayload;
use crate::state::AppState;

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

/// 项目粘性授权规则概览（v2.145 设置管理行）：全部工作区的规则计数。
#[tauri::command]
pub async fn permission_rules_list(
    state: State<'_, AppState>,
) -> Result<Vec<crate::permission_rules::ProjectRulesView>, String> {
    Ok(state.permission_rules.list_all())
}

/// 清除一个工作区的全部粘性规则（v2.145 设置管理行）。
#[tauri::command]
pub async fn permission_rules_delete(
    workspace: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.permission_rules.clear(&workspace)
}

/// 规则目录路径（v2.145 设置管理行「打开文件夹」出口）。
#[tauri::command]
pub async fn permission_rules_dir(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.permission_rules.dir().to_string_lossy().into_owned())
}
