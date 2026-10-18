/** **应用内通知**（v2.51 §3.13）：读 / 全读 / 单读 / 清空 + 新通知推送订阅。
 * 五条都服务通知中心；入账与推送在后端同一处，前端只有这一族出口。 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { NotificationItem } from "../../types";
// ---- 应用内通知（v2.51 §3.13）：读 / 全读 / 单读 / 清空 + 推送订阅 ----

export function notificationList(): Promise<NotificationItem[]> {
  return invoke("notification_list");
}

export function notificationReadAll(): Promise<number> {
  return invoke("notification_read_all");
}

export function notificationRead(id: string): Promise<boolean> {
  return invoke("notification_read", { id });
}

export function notificationClear(): Promise<void> {
  return invoke("notification_clear");
}

/** 新通知推送（后端 TauriSink 入账后 emit）。 */
export function subscribeNotificationAdded(
  onAdded: (item: NotificationItem) => void,
): Promise<UnlistenFn> {
  return listen<NotificationItem>("lw://notification", (e) => onAdded(e.payload));
}
