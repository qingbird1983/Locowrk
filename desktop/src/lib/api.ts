// IPC 封装（DESIGN.md §12.2 前端侧）：命令走 invoke，事件走 listen。
// 命令只做发起，内容全部从 `agent://` 事件来。
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentEvent,
  ApprovalRequest,
  InstalledPlugin,
  LlmUpdate,
  MarketplaceItem,
  PluginStateChange,
  SettingsView,
} from "../types";

/** 发起一轮对话；返回实际 step 数。内容经事件到达，不经返回值。 */
export function chat(sessionId: string, message: string): Promise<number> {
  return invoke("agent_chat", { sessionId, message });
}

/** 回填审批决策（§12.1）。与已处理/未知的请求返回 reject。 */
export function approve(requestId: string, allowed: boolean): Promise<void> {
  return invoke("agent_approve", { requestId, allowed });
}

/** 未决审批补拉（前端刷新/重连时，事件可能已错过）。 */
export function listPendingApprovals(): Promise<ApprovalRequest[]> {
  return invoke("list_pending_approvals");
}

const CHANNELS = [
  "agent://chunk",
  "agent://tool-call",
  "agent://tool-result",
  "agent://turn-done",
] as const;

export function subscribeAgentEvents(
  onEvent: (e: AgentEvent) => void,
): Promise<UnlistenFn[]> {
  return Promise.all(
    CHANNELS.map((ch) => listen<AgentEvent>(ch, (e) => onEvent(e.payload))),
  );
}

export function subscribeApprovals(
  onRequest: (req: ApprovalRequest) => void,
): Promise<UnlistenFn[]> {
  return listen<ApprovalRequest>("agent://approval-request", (e) =>
    onRequest(e.payload),
  ).then((un) => [un]);
}

// ---- 插件管理（DESIGN.md §11.1 / §13.3 / §13.4） ----

/** 已安装列表（lockfile 投影 + 注册表运行态合并）。 */
export function pluginList(): Promise<InstalledPlugin[]> {
  return invoke("plugin_list");
}

/** 市场目录（发现页）。 */
export function pluginRegistryList(): Promise<MarketplaceItem[]> {
  return invoke("plugin_registry_list");
}

/** 从市场安装；version 缺省取 latest。返回实际安装的版本。 */
export function pluginInstall(id: string, version?: string): Promise<string> {
  return invoke("plugin_install", { id, version: version ?? null });
}

/** 启用 / 禁用（热插拔）。 */
export function pluginSetEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke("plugin_set_enabled", { id, enabled });
}

/** 卸载（Windows 文件锁下删目录会延迟到下次启动清理）。 */
export function pluginUninstall(id: string): Promise<void> {
  return invoke("plugin_uninstall", { id });
}

/** 回退到指定版本。返回回退到的版本。 */
export function pluginRollback(id: string, version: string): Promise<string> {
  return invoke("plugin_rollback", { id, version });
}

/** 更新到市场 latest（失败自动回滚）。返回新版本。 */
export function pluginUpdate(id: string): Promise<string> {
  return invoke("plugin_update", { id });
}

/** 订阅插件状态实时同步（§13.4）。 */
export function subscribePluginState(
  onChange: (change: PluginStateChange) => void,
): Promise<UnlistenFn[]> {
  return listen<PluginStateChange>("plugin://state-changed", (e) =>
    onChange(e.payload),
  ).then((un) => [un]);
}

// ---- 设置（DESIGN.md §13.1 / §13.5） ----

/** 读设置投影（永不含 API Key 原值）。 */
export function getSettings(): Promise<SettingsView> {
  return invoke("settings_get");
}

/** 部分更新设置，返回最新投影。 */
export function setSettings(patch: {
  locale?: string;
  llm?: LlmUpdate;
}): Promise<SettingsView> {
  return invoke("settings_set", {
    locale: patch.locale ?? null,
    llm: patch.llm ?? null,
  });
}
