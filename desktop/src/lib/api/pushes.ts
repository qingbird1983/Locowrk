/** **末尾实时推送订阅**（browser_open 工具）：浏览器开签。按"推送订阅"这个形状
 * 立片而不是按消费方——接收方是工作面板，与通用通道表无关。
 *
 * S4 刀② 起整理收束 / 熔断翻转两条（`lw://organize-done` / `-paused`）随记忆面
 * 退役——整理线整条删除，前端不再有它们的订阅面。
 *
 * 通用通道表的订阅入口不在这儿，在 channels 片；本片走专用通道。 */

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { BrowserOpenPayload } from "../../types";
/** 浏览器页签开签推送（lw://browser-open；browser_open 工具 → 工作面板）。 */
export function subscribeBrowserOpen(
  onOpen: (payload: BrowserOpenPayload) => void,
): Promise<UnlistenFn> {
  return listen<BrowserOpenPayload>("lw://browser-open", (e) =>
    onOpen(e.payload),
  );
}
