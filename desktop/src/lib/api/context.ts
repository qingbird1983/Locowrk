/** **上下文压缩的撤销**（⑨）：一条命令、前缀游标语义（其后的轮次一并回退、会话 id 不变）。
 * 单独成片：它与"发起一轮"和"审批回填"都不是一件事，而原文就把它排在这两段中间。 */

import { invoke } from "@tauri-apps/api/core";
/** 撤销最近一次上下文压缩（⑨）：其后的轮次一并回退（前缀游标语义）。
 * 会话 id 不变。返回回退点。 */
export function contextUndoCompact(sessionId: string): Promise<number> {
  return invoke("context_undo_compact", { sessionId });
}
