/** **上下文压缩的人手侧**（⑨ 撤销 + S2 刀⑦ 手动压缩）：两条同族——都是"人对这一层
 * 主动做的动作"，与"发起一轮"和"审批回填"都不是一件事。
 *
 * 刀⑦ 之前这一族只有撤销那一条，而 `/整理` 的手动触发寄居在 `developer.ts`（那片头注
 * 当时留了一句"挪它 = 语义改动，另刀另证"——就是这一刀）。现在它换成 `compact_now`
 * 并回到本族：压缩的手动入口与它的撤销口住一处，开发者那一片重新只管"读数"。 */

import { invoke } from "@tauri-apps/api/core";
import type { CompactNowView } from "../../types";
/** 撤销最近一次上下文压缩（⑨）：其后的轮次一并回退（前缀游标语义）。
 * 会话 id 不变。返回回退点。 */
export function contextUndoCompact(sessionId: string): Promise<number> {
  return invoke("context_undo_compact", { sessionId });
}

/** `/压缩`（S2 刀⑦）：当场跑一次主层深度压缩。后端不受触发线与熔断限制，
 * 但**窗口未知 / 形状不够**时照旧如实拒（判据见 `compact::plan` 的 `manual` 段）。
 * 压成了什么也不在这儿返回——那条断层走 `agent://context-compacted`。 */
export function compactNow(sessionId: string): Promise<CompactNowView> {
  return invoke("compact_now", { sessionId });
}
