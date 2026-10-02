// 工作面板状态（UI shell 文档 §2.5，v2.52）：开合 / 宽度 / 最大化 / 页签
// ——**跨会话保持的壳状态**（切会话不丢），宽度与开合持久化（localStorage），
// 页签进程内不跨重启（§3.16 转录文件在，可重开）。宽度上限 = 三栏预算
// （容器宽 − 侧栏宽 − 主区地板 450），由 WorkPanel 拖柄实时钳制。
import { defineStore } from "pinia";
import {
  loadPanelPrefs,
  savePanelPrefs,
  PANEL_DEFAULT,
  PANEL_MIN,
} from "../lib/work-panel-preference";

export interface WorkPanelTab {
  /** 页签 id = 子会话 id（§3.16 唯一内置页签类型）。 */
  id: string;
  kind: "subagent";
  /** 页签名 = 子 Agent + 任务 prompt 截断摘要（⑮ 无 name 参数，不发明）。 */
  title: string;
}

const prefs = loadPanelPrefs();

export const useWorkPanelStore = defineStore("workPanel", {
  state: () => ({
    open: prefs.open,
    width: prefs.width,
    /** 最大化 = 宽取当前预算最大（主区保持地板）；还原回先前宽度。 */
    maximized: false,
    widthBeforeMax: prefs.width,
    tabs: [] as WorkPanelTab[],
    activeTabId: null as string | null,
  }),
  actions: {
    persist() {
      savePanelPrefs({ open: this.open, width: this.width });
    },
    setOpen(v: boolean) {
      this.open = v;
      this.persist();
    },
    toggle() {
      this.setOpen(!this.open);
    },
    /** 拖拽/键盘/双击落定的宽度（调用方已 clamp 到预算）。 */
    setWidth(w: number) {
      this.width = Math.max(1, Math.round(w));
      if (this.maximized) this.maximized = false;
      this.persist();
    },
    /** 预算内钳制（WorkPanel 持有预算计算，入口处统一收口）。 */
    clampTo(budget: number) {
      if (budget < PANEL_MIN) return Math.max(1, budget);
      if (this.maximized) return budget;
      return Math.min(Math.max(this.width, PANEL_MIN), budget);
    },
    toggleMaximized(budget: number) {
      if (this.maximized) {
        this.maximized = false;
        this.width = this.widthBeforeMax;
      } else {
        this.maximized = true;
        this.widthBeforeMax = this.width;
        this.width = Math.max(1, Math.round(budget));
      }
      this.persist();
    },
    openSubTab(childId: string, title: string) {
      if (!this.tabs.some((t) => t.id === childId)) {
        this.tabs.push({ id: childId, kind: "subagent", title });
      }
      this.activeTabId = childId;
      this.open = true;
      this.persist();
    },
    closeTab(id: string) {
      const idx = this.tabs.findIndex((t) => t.id === id);
      if (idx === -1) return;
      this.tabs.splice(idx, 1);
      if (this.activeTabId === id) {
        this.activeTabId = this.tabs[Math.min(idx, this.tabs.length - 1)]?.id ?? null;
      }
      if (this.tabs.length === 0) this.activeTabId = null;
    },
    resetWidth() {
      this.maximized = false;
      this.width = PANEL_DEFAULT;
      this.persist();
    },
  },
});
