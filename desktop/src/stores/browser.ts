// 浏览器页签 console 账本前端侧（UI shell 文档 §2.5 浏览器页签，v2.144）：
// 预览页注入桥 postMessage 的 error/warn 在这里按页签收集（展示 + 徽标），
// 并批量推回后端 browser_report_errors（模型 browser_console 工具的读源；
// 后端再钳 200 条环形，双端同宽）。纯会话内状态：页签关 = 前端缓冲随签
// 清（后端留底供模型读，宁留勿丢）。
import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface BrowserConsoleEntry {
  ts: number;
  level: "error" | "warn";
  text: string;
}

/** 前端环形上限（与后端 PER_TAB_CAP 同宽）。 */
const RING_CAP = 200;
/** 回传节流：每页签至多 500ms 一批（吵闹页面不刷爆 IPC）。 */
const REPORT_INTERVAL_MS = 500;

export const useBrowserStore = defineStore("browserPanel", {
  state: () => ({
    entries: {} as Record<string, BrowserConsoleEntry[]>,
    /** 已推回后端的条数（每签游标：只发增量，环形 shift 时同步回调）。 */
    reported: {} as Record<string, number>,
  }),
  actions: {
    push(tabId: string, entry: BrowserConsoleEntry) {
      const buf = (this.entries[tabId] ??= []);
      if (buf.length >= RING_CAP) {
        buf.shift();
        if ((this.reported[tabId] ?? 0) > 0) this.reported[tabId]!--;
      }
      buf.push(entry);
      this.scheduleReport(tabId);
    },
    /** 页签关闭：前端缓冲随签清（后端账本留底）。 */
    drop(tabId: string) {
      delete this.entries[tabId];
      delete this.reported[tabId];
      const t = reportTimers.get(tabId);
      if (t !== undefined) {
        window.clearTimeout(t);
        reportTimers.delete(tabId);
      }
    },
    scheduleReport(tabId: string) {
      if (reportTimers.has(tabId)) return;
      const t = window.setTimeout(() => {
        reportTimers.delete(tabId);
        void this.report(tabId);
      }, REPORT_INTERVAL_MS);
      reportTimers.set(tabId, t);
    },
    async report(tabId: string) {
      const buf = this.entries[tabId] ?? [];
      const fresh = buf.slice(this.reported[tabId] ?? 0);
      if (fresh.length === 0) return;
      this.reported[tabId] = buf.length;
      try {
        await invoke("browser_report_errors", {
          tabId,
          report: { entries: fresh },
        });
      } catch {
        // 后端缺席（理论不可能）不打断展示面；下一批照常再试
      }
    },
  },
});

/** 节流定时器表（模块级，不进响应式状态）。 */
const reportTimers = new Map<string, number>();
