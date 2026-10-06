// 壳状态（UI shell doc §2.4/§2.6）：目的地路由 + 侧栏宽/折叠 + 设置当前分区
// （§3.32 v3.4 板面切换模型：右栏一次只渲染一个分区，轨点击与 ⌘K 命中都改
// 写同一份状态）。目的地切换的保活规则在 App.vue（对话 hidden+inert 挂起；
// 其他目的地按需挂载）。§3.40（v2.109）起折叠 = 手动位 + 视口断点两层：
// manualSidebarCollapsed 是用户意图（持久化、断点永不改写），viewport*Ok
// 由 use-responsive-shell 写入；sidebarCollapsed 为合成 getter。
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
    /** 用户手动折叠位（§3.40②·4：持久化；断点级联永不改写——收窄折叠
     * ≠ 用户折叠）。展示态见 getter sidebarCollapsed。 */
    manualSidebarCollapsed: prefs.collapsed,
    /** 视口旗标（§3.40，use-responsive-shell 写入；模块期先按当前窗口
     * 预置，挂载后校准——首帧无闪变）。 */
    viewportLeftOk: window.innerWidth >= 1100,
    viewportRightOk: window.innerWidth >= 1400,
    tooNarrow: window.innerWidth < 800,
    /** 设置当前分区（§3.32 板面切换模型；重开设置页保持上次看过的板面）。 */
    settingsSection: "appearance" as SettingsSectionId,
    /** composer 聚焦令牌（§3.17：决策后焦点回输入框——批完立刻能接着
     * 打字）。PermissionCard 决策成功后自增，Composer watch 聚焦。 */
    composerFocusTick: 0,
    /** 命令面板开合（§3.15 v3.9：侧栏「搜索」行与 Ctrl+K 同流一实例）。 */
    paletteOpen: false,
  }),
  getters: {
    /** 展示态 = 手动折叠 ‖ 视口不允许（§3.40①：md 档左栏级联折叠）。 */
    sidebarCollapsed(state): boolean {
      return state.manualSidebarCollapsed || !state.viewportLeftOk;
    },
  },
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
        collapsed: this.manualSidebarCollapsed,
      });
    },
    /** 折叠/展开唯一写入口：只翻手动位（视口旗标归 composable）。 */
    toggleSidebar() {
      this.manualSidebarCollapsed = !this.manualSidebarCollapsed;
      this.commitSidebarWidth();
    },
    /** 命令面板（§3.15）：Ctrl+K 与侧栏「搜索」行共用同一开合态。 */
    openPalette() {
      this.paletteOpen = true;
    },
    closePalette() {
      this.paletteOpen = false;
    },
    /** 视口旗标落位（§3.40①断点表；width clamp 是纯偏好修正，与折叠无关）。 */
    applyViewportWidth(w: number) {
      this.viewportLeftOk = w >= 1100;
      this.viewportRightOk = w >= 1400;
      this.tooNarrow = w < 800;
      const max = Math.min(SIDEBAR_MAX, w - 320);
      if (this.sidebarWidth > max) {
        this.sidebarWidth = Math.max(SIDEBAR_MIN, Math.round(max));
      }
    },
  },
});
