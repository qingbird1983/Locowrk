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
import { BREAKPOINT_MD } from "../lib/breakpoints";
import { useShellStore } from "./shell";

export interface WorkPanelTab {
  /** 页签 id：子会话 = childId（§3.16）；审阅 = `rev:<callId>`（§3.28）。 */
  id: string;
  kind: "subagent" | "review";
  /** 子页签名 = 子 Agent + 任务 prompt 截断；审阅页签名 = 文件路径末段。 */
  title: string;
  /** 审阅页签数据快照（edit_diff 的 unified diff，进程内跨会话保持）。 */
  path?: string;
  diff?: string;
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
      // §3.40②·5（v2.111 V2 规格）：任何宽度都可合法开（v2.110 的 ≥1400
      // 钳制随「按钮永不冻结」退役）；overlay 档（400–799）互斥——开右栏
      // 把左栏挂瞬态折叠（不落盘，回宽自动恢复），关右栏解除瞬态。
      this.open = v;
      if (window.innerWidth < BREAKPOINT_MD) {
        useShellStore().setPanelExclusiveFold(v);
      }
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
      this.openTab({ id: childId, kind: "subagent", title });
    },
    /** 审阅页签（§3.28）：同 callId 重开 = 快照覆盖（文件最新态）。 */
    openReviewTab(callId: string, path: string, diff: string) {
      const id = `rev:${callId}`;
      const idx = this.tabs.findIndex((t) => t.id === id);
      const tab: WorkPanelTab = {
        id,
        kind: "review",
        title: path.split(/[\\/]/).pop() ?? path,
        path,
        diff,
      };
      if (idx === -1) this.tabs.push(tab);
      else this.tabs.splice(idx, 1, tab);
      this.activeTabId = id;
      this.setOpen(true);
    },
    openTab(tab: WorkPanelTab) {
      if (!this.tabs.some((t) => t.id === tab.id)) {
        this.tabs.push(tab);
      }
      this.activeTabId = tab.id;
      this.setOpen(true);
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
