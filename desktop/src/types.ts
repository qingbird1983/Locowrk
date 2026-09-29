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
