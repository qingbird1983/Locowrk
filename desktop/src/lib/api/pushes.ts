/** **末尾三条实时推送订阅**（§3.37③ / browser_open 工具 / 熔断翻转）：整理收束、
 * 浏览器开签、整理熔断翻转。按"推送订阅"这个形状立片而不是按消费方——三条的接收方
 * 分别是 chat store 与工作面板，没有共同上位功能名。
 *
 * 通用通道表的订阅入口不在这儿，在 channels 片；本片这三条走各自专用通道。 */

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BrowserOpenPayload,
  OrganizeDonePayload,
  OrganizePausedPayload,
} from "../../types";
/** 整理收束推送（§3.37③；chat store 换 SystemNotice，非当前会话忽略）。 */
export function subscribeOrganizeDone(
  onDone: (payload: OrganizeDonePayload) => void,
): Promise<UnlistenFn> {
  return listen<OrganizeDonePayload>("lw://organize-done", (e) =>
    onDone(e.payload),
  );
}

/** 浏览器页签开签推送（lw://browser-open；browser_open 工具 → 工作面板）。 */
export function subscribeBrowserOpen(
  onOpen: (payload: BrowserOpenPayload) => void,
): Promise<UnlistenFn> {
  return listen<BrowserOpenPayload>("lw://browser-open", (e) =>
    onOpen(e.payload),
  );
}

/** 熔断翻转推送（仅 paused_auto 翻转瞬间发）。 */
export function subscribeOrganizePaused(
  onPaused: (payload: OrganizePausedPayload) => void,
): Promise<UnlistenFn> {
  return listen<OrganizePausedPayload>("lw://organize-paused", (e) =>
    onPaused(e.payload),
  );
}
