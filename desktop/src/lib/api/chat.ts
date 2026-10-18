/** **发起一轮对话**（DESIGN.md §12.2 前端侧）：本片只有 chat() 一条 —— 它单独成片是因为
 * 「内容全部从事件来、返回值只带 step 数」这条纪律就住在它身上，附件面与压缩撤销面各在
 * assets 与 context 两片（同一片装三件事 = 违三问 ①，判定表给的合并切法按簇拆开）。
 *
 * 公开名由本文件原样再导出，二十余处消费点的 import 路径一个字不改。 */

import { invoke } from "@tauri-apps/api/core";
import type { ContentPartDto } from "../../types";
/** 发起一轮对话；返回实际 step 数。内容经事件到达，不经返回值。
 * workspace = 首聊挂起归属（㉘·A·fix 归属先行）：后端建会话即写旁车，
 * 先于首轮注入/围栏/提取——不再等轮次收束后才关联。
 * parts（批 D 多模态）：附件部件（assetPut/assetPutPath 已上传的引用）。 */
export function chat(
  sessionId: string,
  message: string,
  workspace?: string | null,
  parts?: ContentPartDto[],
): Promise<number> {
  return invoke("agent_chat", {
    sessionId,
    message,
    workspace: workspace ?? null,
    parts: parts?.length ? parts : null,
  });
}
