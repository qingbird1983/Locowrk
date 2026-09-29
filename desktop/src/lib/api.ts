// IPC 封装（DESIGN.md §12.2 前端侧）：命令走 invoke，事件走 listen。
// 命令只做发起，内容全部从 `agent://` 事件来。
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentEvent, ApprovalRequest } from "../types";

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
