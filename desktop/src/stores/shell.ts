// 壳状态（UI shell doc §2.4/§2.6）：目的地路由 + 侧栏宽/折叠 + 设置当前分区
// （§3.32 v3.4 板面切换模型：右栏一次只渲染一个分区，轨点击与 ⌘K 命中都改
// 写同一份状态）。目的地切换的保活规则在 App.vue（对话 hidden+inert 挂起；
// 其他目的地按需挂载）。
import { defineStore } from "pinia";
import {
  loadSidebarPrefs,
  saveSidebarPrefs,
  SIDEBAR_MAX,
  SIDEBAR_MIN,
} from "../lib/sidebar-preference";
import type { SettingsSectionId } from "../lib/settings-index";

export type Destination = "chat" | "plugins" | "memory" | "schedule" | "settings";

const prefs = loadSidebarPrefs();

export const useShellStore = defineStore("shell", {
  state: () => ({
    /** 当前目的地（§2.4：对话 = 隐式入口，侧栏即对话列表）。 */
    destination: "chat" as Destination,
    /** 侧栏展开宽（实时值；写 --lw-sidebar-w，§6.4）。 */
    sidebarWidth: prefs.width,
    /** 折叠是独立状态（§2.3：折叠 ≠ 宽度 0，展开回到记住的宽度）。 */
    sidebarCollapsed: prefs.collapsed,
    /** 设置当前分区（§3.32 板面切换模型；重开设置页保持上次看过的板面）。 */
    settingsSection: "appearance" as SettingsSectionId,
    /** composer 聚焦令牌（§3.17：决策后焦点回输入框——批完立刻能接着
     * 打字）。PermissionCard 决策成功后自增，Composer watch 聚焦。 */
    composerFocusTick: 0,
  }),
  actions: {
    setDestination(dest: Destination) {
      this.destination = dest;
    },
    /** 打开设置（可选带目标分区）：⌘K 命中与轨内点击共用同一条入口。 */
    openSettings(section?: SettingsSectionId) {
      if (section) this.settingsSection = section;
      this.destination = "settings";
    },
    /** 拖拽中实时调宽（不落盘，up 才 commit）。上限随视口收口。 */
    setSidebarWidth(w: number) {
      const max = Math.min(SIDEBAR_MAX, window.innerWidth - 320);
      this.sidebarWidth = Math.round(Math.min(max, Math.max(SIDEBAR_MIN, w)));
    },
    commitSidebarWidth() {
      // 读改写：保留 SessionGroups 持有的分组头折叠键（v2.50）
      saveSidebarPrefs({
        ...loadSidebarPrefs(),
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
