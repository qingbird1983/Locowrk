/** **审批回填与项目粘性规则**（§12.1 / §3.17 / v2.145 设置管理行）：决策回填、规则概览与
 * 清除、规则目录出口、未决审批补拉。ProjectRulesView 这个形状就住在本片（只有这几条读它）。
 *
 * 规则四条与回填一条同片，是因为它们回答的是同一个问题：**这次副作用允不允许、以什么范围
 * 允许**——范围值（session / project）就写在这两族之间。 */

import { invoke } from "@tauri-apps/api/core";
import type { ApprovalRequest } from "../../types";
/** 回填审批决策（§12.1 / §3.17）。scope="session" 且放行 = 允许本次对话
 * （豁免集记在后端 ApprovalHub，本会话后续同工具不再问）。
 * 与已处理/未知的请求返回 reject（含超时已被后端自动拒绝的请求）。 */
export function approve(
  requestId: string,
  allowed: boolean,
  scope?: "session" | "project",
): Promise<void> {
  return invoke("agent_approve", { requestId, allowed, scope: scope ?? null });
}

// ---- 项目粘性授权规则（v2.145 设置管理行）----

/** 单工作区规则概览（permission_rules_list 投影）。 */
export interface ProjectRulesView {
  workspace: string;
  allow: number;
  deny: number;
}

/** 全部工作区的规则概览。 */
export function listProjectRules(): Promise<ProjectRulesView[]> {
  return invoke("permission_rules_list");
}

/** 清除一个工作区的全部粘性规则。 */
export function deleteProjectRules(workspace: string): Promise<void> {
  return invoke("permission_rules_delete", { workspace });
}

/** 规则目录路径（「打开文件夹」出口）。 */
export function projectRulesDir(): Promise<string> {
  return invoke("permission_rules_dir");
}

/** 未决审批补拉（前端刷新/重连时，事件可能已错过）。 */
export function listPendingApprovals(): Promise<ApprovalRequest[]> {
  return invoke("list_pending_approvals");
}
