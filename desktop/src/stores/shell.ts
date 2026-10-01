// 壳状态（UI shell doc §2.4/§2.6）：目的地路由 + 侧栏宽/折叠 + ⌘K 的设置
// 分区跳转载荷（§3.15/§3.32：命中设置类结果 → 进设置并滚到该分区闪一下）。
// 目的地切换的保活规则在 App.vue（对话 hidden+inert 挂起；其他目的地按需
// 挂载）。
import { defineStore } from "pinia";
import {
  loadSidebarPrefs,
  saveSidebarPrefs,
  SIDEBAR_MAX,
  SIDEBAR_MIN,
} from "../lib/sidebar-preference";
import type { SettingsSectionId } from "../lib/settings-index";

export type Destination = "chat" | "plugins" | "memory" | "settings";

export interface SettingsFocus {
  section: SettingsSectionId;
  /** 每次跳转自增：同分区连跳两次也能触发 watcher。 */
  nonce: number;
}

const prefs = loadSidebarPrefs();

export const useShellStore = defineStore("shell", {
  state: () => ({
    /** 当前目的地（§2.4：对话 = 隐式入口，侧栏即对话列表）。 */
    destination: "chat" as Destination,
    /** 侧栏展开宽（实时值；写 --lw-sidebar-w，§6.4）。 */
    sidebarWidth: prefs.width,
    /** 折叠是独立状态（§2.3：折叠 ≠ 宽度 0，展开回到记住的宽度）。 */
    sidebarCollapsed: prefs.collapsed,
    /** ⌘K 设置跳转载荷（§3.32 三）；null = 无待跳转。 */
    settingsFocus: null as SettingsFocus | null,
  }),
  actions: {
    setDestination(dest: Destination) {
      this.destination = dest;
    },
    /** ⌘K 命中设置分区：切目的地 + 记下待滚分区（SettingsPage 消费）。 */
    focusSettingsSection(section: SettingsSectionId) {
      this.destination = "settings";
      const prev = this.settingsFocus;
      this.settingsFocus = { section, nonce: (prev?.nonce ?? 0) + 1 };
    },
    /** 拖拽中实时调宽（不落盘，up 才 commit）。上限随视口收口。 */
    setSidebarWidth(w: number) {
      const max = Math.min(SIDEBAR_MAX, window.innerWidth - 320);
      this.sidebarWidth = Math.round(Math.min(max, Math.max(SIDEBAR_MIN, w)));
    },
    commitSidebarWidth() {
      saveSidebarPrefs({
        width: this.sidebarWidth,
        collapsed: this.sidebarCollapsed,
      });
    },
    toggleSidebar() {
      this.sidebarCollapsed = !this.sidebarCollapsed;
      this.commitSidebarWidth();
    },
    /** 拖到小于最小值即折叠（§2.3：把"拖没了"变成明确状态）。 */
    collapseFromResize() {
      if (this.sidebarCollapsed) return;
      this.sidebarCollapsed = true;
      this.commitSidebarWidth();
    },
    applyViewport() {
      // 视口收窄后旧宽度超上限：实时钳制（不折叠——那是用户手势的事）
      const max = Math.min(SIDEBAR_MAX, window.innerWidth - 320);
      if (this.sidebarWidth > max) {
        this.sidebarWidth = Math.max(SIDEBAR_MIN, Math.round(max));
      }
    },
  },
});
