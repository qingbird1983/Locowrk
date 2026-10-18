/** **ask_user 应答面**（§3.39①）：回填应答或跳过、卡内首次交互续期、未决补拉。
 * 三条都以 ask 开头、都服务同一张选择卡。 */

import { invoke } from "@tauri-apps/api/core";
import type { AgentEvent } from "../../types";
/** 回填 ask_user 应答（§3.39①）：answer = 选择 + 自由补充；skip = 跳过。
 * 请求已收束（超时/停止/重复应答）= reject，前端据此摘卡。 */
export function askRespond(
  requestId: string,
  answer: { kind: "answer"; labels: string[]; note?: string } | { kind: "skip" },
): Promise<void> {
  return invoke("ask_respond", {
    requestId,
    answer:
      answer.kind === "answer"
        ? { kind: "answer", labels: answer.labels, note: answer.note ?? "" }
        : { kind: "skip", reason: null },
  });
}

/** ask 卡内首次交互续期：摘除本次守夜（打字不被超时吞）。幂等。 */
export function askSnooze(requestId: string): Promise<void> {
  return invoke("ask_snooze", { requestId });
}

/** 未决 ask 补拉（挂载/重连/刷新时）。 */
export function listPendingAsks(): Promise<AgentEvent[]> {
  return invoke("list_pending_asks");
}
