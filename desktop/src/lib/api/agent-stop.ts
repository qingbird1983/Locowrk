/** **停止生成**（§3.39④）：后端三路同收（停止标记 + 挂起审批 fail-closed 拒绝 + 待决卡按
 * 跳过收束），幂等。单独成片 = 它同时跨审批与 ask 两个域，塞进任何一边都会让另一边的名字撒谎。 */

import { invoke } from "@tauri-apps/api/core";
/** 停止生成（§3.39④）：后端三路同收——停止标记置位（流式带部分正文
 * 收束）+ 挂起审批 fail-closed 拒绝 + 待决 ask 卡按跳过收束。幂等。 */
export function agentStop(sessionId: string): Promise<void> {
  return invoke("agent_stop", { sessionId });
}
