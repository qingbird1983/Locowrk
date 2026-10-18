/** **自动会话标题的推送**（v2.57）：旁车落盘后通知，前端据此更新侧栏。
 * 它排在定时段之后、且只有一条订阅 ⇒ 不并入定时片（那条命令的载荷与调度无关）。 */

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SessionTitlePayload } from "../../types";
/** 自动会话标题推送（v2.57）：旁车落盘后通知（chat store 更新侧栏）。 */
export function subscribeSessionTitle(
  onTitle: (payload: SessionTitlePayload) => void,
): Promise<UnlistenFn> {
  return listen<SessionTitlePayload>("lw://session-title", (e) =>
    onTitle(e.payload),
  );
}
