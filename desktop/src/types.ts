/**
 * `agent://` 事件的 TS 镜像类型。
 * 与 desktop/src-tauri/src/events.rs 的 AgentEvent 保持一致（snake_case 字段）。
 */
export type AgentEvent =
  | ChunkEvent
  | ToolCallEvent
  | ToolResultEvent
  | TurnDoneEvent;

export interface ChunkEvent {
  kind: "chunk";
  session_id: string;
  delta: string;
}

export interface ToolCallEvent {
  kind: "tool_call";
  session_id: string;
  call_id: string;
  tool: string;
  args: unknown;
  side_effect: string;
  plugin_version: string;
}

export interface ToolResultEvent {
  kind: "tool_result";
  session_id: string;
  call_id: string;
  ok: boolean;
  output: unknown;
  error: string | null;
}

export interface TurnDoneEvent {
  kind: "turn_done";
  session_id: string;
  steps: number;
}

/** 审批请求载荷（agent://approval-request）。与 events.rs 的
 * ApprovalRequestPayload 对齐（snake_case 字段）。 */
export interface ApprovalRequest {
  request_id: string;
  tool: string;
  args: unknown;
  side_effect: string;
}

/** 市场目录条目（plugin_registry_list 返回项；versions 升序，末位 latest）。 */
export interface MarketplaceItem {
  id: string;
  description: string;
  versions: string[];
}

/** 已安装插件（lockfile 投影 + 注册表运行态合并）。 */
export interface InstalledPlugin {
  id: string;
  version: string;
  enabled: boolean;
  installed_at: number;
  source: string;
  active_version: string | null;
  loaded_versions: string[];
}

/** plugin://state-changed 载荷（plugin-service PluginStateChange 镜像，
 * type 标签）。 */
export type PluginStateChange =
  | { type: "installed"; id: string; version: string }
  | { type: "enabled_changed"; id: string; enabled: boolean }
  | { type: "uninstalled"; id: string }
  | { type: "active_version_changed"; id: string; version: string };
