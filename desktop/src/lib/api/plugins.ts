/** **插件管理面**（DESIGN.md §11.1 / §13.3 / §13.4）：列表与带使用统计的列表、批量启停与
 * 卸载、市场目录、安装/回退/更新、单条启停卸载、状态实时同步订阅 —— 十一条都在插件这一族。 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BatchResult,
  InstalledPlugin,
  MarketplaceItem,
  PluginStateChange,
  PluginUsageView,
} from "../../types";
// ---- 插件管理（DESIGN.md §11.1 / §13.3 / §13.4） ----

/** 已安装列表（lockfile 投影 + 注册表运行态合并）。 */
export function pluginList(): Promise<InstalledPlugin[]> {
  return invoke("plugin_list");
}

/** 带使用统计的已安装列表（⑩⑤）：闲置分级后端裁决，按 id join。 */
export function pluginListWithUsage(): Promise<PluginUsageView[]> {
  return invoke("plugin_list_with_usage");
}

/** 批量启用/停用（⑩⑥）：逐条成败，部分失败不回滚。 */
export function pluginBatchSetEnabled(
  ids: string[],
  enabled: boolean,
): Promise<BatchResult> {
  return invoke("plugin_batch_set_enabled", { ids, enabled });
}

/** 批量卸载（⑩⑥）：调用方负责二次确认并列出插件名（§9.5）。 */
export function pluginBatchUninstall(ids: string[]): Promise<BatchResult> {
  return invoke("plugin_batch_uninstall", { ids });
}

/** 市场目录（发现页）。 */
export function pluginRegistryList(): Promise<MarketplaceItem[]> {
  return invoke("plugin_registry_list");
}

/** 从市场安装；version 缺省取 latest。返回实际安装的版本。 */
export function pluginInstall(id: string, version?: string): Promise<string> {
  return invoke("plugin_install", { id, version: version ?? null });
}

/** 启用 / 停用（热插拔）。 */
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
