// 壳状态（UI shell doc §2.4/§2.6）：目的地路由 + 侧栏宽/折叠 + 设置当前分区
// （§3.32 v3.4 板面切换模型：右栏一次只渲染一个分区，轨点击与 ⌘K 命中都改
// 写同一份状态）。目的地切换的保活规则在 App.vue（对话 hidden+inert 挂起；
// 其他目的地按需挂载）。§3.40（v2.112 用户口述定稿）折叠分档合成：
// - push 档：手动位（持久化，自动化永不改写）‖ 级联瞬态 sidebarAutoFold
//   （跌破 1100 沿触发，不落盘；回升自动解除）；
// - overlay 档（400–799）：浮层展开态 overlaySidebarOpen ‖ 互斥瞬态
//   overlaySidebarFold——带内默认全收，展开 = 悬浮式一次一边。
// manualSidebarCollapsed 是用户意图；sidebarCollapsed 为分档 getter。
import { defineStore } from "pinia";
import {
  loadSidebarPrefs,
  saveSidebarPrefs,
  SIDEBAR_MAX,
  SIDEBAR_MIN,
} from "../lib/sidebar-preference";
import { BREAKPOINT_LG, BREAKPOINT_MD, BREAKPOINT_SM } from "../lib/breakpoints";
import { useWorkPanelStore } from "./work-panel";
import type { SettingsSectionId } from "../lib/settings-index";

export type Destination = "chat" | "plugins" | "memory" | "schedule" | "settings";

const prefs = loadSidebarPrefs();

export const useShellStore = defineStore("shell", {
  state: () => ({
    /** 当前目的地（§2.4：对话 = 隐式入口，侧栏即对话列表）。 */
    destination: "chat" as Destination,
    /** 侧栏展开宽（实时值；写 --lw-sidebar-w，§6.4）。 */
    sidebarWidth: prefs.width,
    /** 用户手动折叠位（持久化；任何自动化永不改写——级联/互斥折叠 ≠ 用户
     * 折叠）。仅 push 档消费（overlay 档看浮层展开态）。 */
    manualSidebarCollapsed: prefs.collapsed,
    /** 左栏级联瞬态折叠（§3.40②·2）：跌破 1100 沿触发挂上，不落盘——
     * 回升 >1100 自动解除（「自动折叠自动恢复」）。 */
    sidebarAutoFold: window.innerWidth < BREAKPOINT_LG,
    /** overlay 档旗标（400–799 浮层档；use-responsive-shell 写入；模块期
     * 按当前窗口预置，挂载后校准——首帧无闪变）。 */
    overlayMode:
      window.innerWidth >= BREAKPOINT_SM && window.innerWidth < BREAKPOINT_MD,
    /** overlay 档左栏浮层展开态（瞬态不落盘；进带复位为收）。 */
    overlaySidebarOpen: false,
    /** overlay 互斥的左栏瞬态折叠（§3.40④）：带内展开右栏时挂上，关右栏
     * 自动解除——左栏浮层态保留，回宽由级联管辖。 */
    overlaySidebarFold: false,
    /** 设置当前分区（§3.32 板面切换模型；重开设置页保持上次看过的板面）。 */
    settingsSection: "appearance" as SettingsSectionId,
    /** composer 聚焦令牌（§3.17：决策后焦点回输入框——批完立刻能接着
     * 打字）。PermissionCard 决策成功后自增，Composer watch 聚焦。 */
    composerFocusTick: 0,
    /** 命令面板（§3.15）：Ctrl+K 与侧栏「搜索」行共用同一开合态。 */
    paletteOpen: false,
  }),
  getters: {
    /** 展示态分档（§3.40①）：overlay 带 = 浮层展开态 ‖ 互斥瞬态；
     * push = 手动位 ‖ 级联瞬态。 */
    sidebarCollapsed(state): boolean {
      if (state.overlayMode) {
        return !state.overlaySidebarOpen || state.overlaySidebarFold;
      }
      return state.manualSidebarCollapsed || state.sidebarAutoFold;
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
    /** 折叠/展开唯一写入口（侧栏头钮 / 顶栏 lead 钮 / Ctrl+B / ⌘K / 遮罩 /
     * 拖柄全走这里）。分档（§3.40②④）：overlay 带翻浮层态（开左栏 =
     * 右栏真实关闭 + 意图消费——互斥方向不对称）；push 带被级联折叠时的
     * 展开请求 = 推翻级联（清瞬态、停靠展开），否则翻手动位。 */
    toggleSidebar() {
      if (this.overlayMode) {
        if (this.overlaySidebarOpen) {
          this.overlaySidebarOpen = false;
          return;
        }
        this.overlaySidebarOpen = true;
        if (window.innerWidth < BREAKPOINT_MD) useWorkPanelStore().setOpen(false);
        return;
      }
      if (!this.manualSidebarCollapsed && this.sidebarAutoFold) {
        this.sidebarAutoFold = false;
        return;
      }
      this.manualSidebarCollapsed = !this.manualSidebarCollapsed;
      this.commitSidebarWidth();
    },
    /** overlay 互斥瞬态旗标读写（唯一调用方 = work-panel.setOpen）。 */
    setPanelExclusiveFold(fold: boolean) {
      this.overlaySidebarFold = fold;
    },
    /** overlay 浮层态复位（进带 = 两栏全收「到达 800 只剩主区」；出带 =
     * 左栏回级联管辖）。唯一调用方 = use-responsive-shell 沿检测。 */
    resetOverlayState() {
      this.overlaySidebarOpen = false;
      this.overlaySidebarFold = false;
    },
    /** 命令面板（§3.15）：Ctrl+K 与侧栏「搜索」行共用同一开合态。 */
    openPalette() {
      this.paletteOpen = true;
    },
    closePalette() {
      this.paletteOpen = false;
    },
    /** 视口旗标落位（§3.40①；width clamp 是纯偏好修正，与折叠无关）。 */
    applyViewportWidth(w: number) {
      this.overlayMode = w >= BREAKPOINT_SM && w < BREAKPOINT_MD;
      if (w >= BREAKPOINT_MD) {
        const max = Math.min(SIDEBAR_MAX, w - 320);
        if (this.sidebarWidth > max) {
          this.sidebarWidth = Math.max(SIDEBAR_MIN, Math.round(max));
        }
      }
    },
  },
});
