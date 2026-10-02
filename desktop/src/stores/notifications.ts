// 应用内通知（UI shell 文档 §3.13，v2.51）：后端 notifications.json 自持
// 一份「用户可能错过的事」，本 store 只消费——打开浮层前 load 全量，
// 运行期订阅 `lw://notification` 收新记录（后端入账即推）。已读/清空是
// 后端命令 + 本地同步更新。
import { defineStore } from "pinia";
import {
  notificationClear,
  notificationList,
  notificationRead,
  notificationReadAll,
  subscribeNotificationAdded,
} from "../lib/api";
import type { NotificationItem } from "../types";

export const useNotificationsStore = defineStore("notifications", {
  state: () => ({
    /** 新→旧（与后端 list 口径一致）。 */
    items: [] as NotificationItem[],
    initialized: false,
  }),
  getters: {
    unread: (s) => s.items.filter((i) => !i.read).length,
  },
  actions: {
    /** App 装载时一次：拉全量 + 订阅推送。 */
    async init() {
      if (this.initialized) return;
      this.initialized = true;
      await this.load();
      await subscribeNotificationAdded((item) => this.items.unshift(item));
    },
    async load() {
      try {
        this.items = await notificationList();
      } catch {
        /* 列表拉取失败不打断启动 */
      }
    },
    async markAllRead() {
      await notificationReadAll();
      for (const i of this.items) i.read = true;
    },
    async markRead(id: string) {
      await notificationRead(id);
      const item = this.items.find((i) => i.id === id);
      if (item) item.read = true;
    },
    async clear() {
      await notificationClear();
      this.items = [];
    },
  },
});
