// 壳状态（UI shell doc §2.4/§2.6）：目的地路由 + 侧栏宽/折叠 + 设置当前分区
// （§3.32 v3.4 板面切换模型：右栏一次只渲染一个分区，轨点击与 ⌘K 命中都改
// 写同一份状态）。目的地切换的保活规则在 App.vue（对话 hidden+inert 挂起；
// 其他目的地按需挂载）。§3.40（v2.111 V2 规格）折叠 = 手动位 + overlay 互斥
// 瞬态旗标两层：manualSidebarCollapsed 是用户意图（持久化，任何自动化永不
// 改写），overlayMode/tooNarrow 由 use-responsive-shell 写入；
// sidebarCollapsed 为合成 getter。v2.109 的 viewportLeftOk/viewportRightOk
// 旗标退役——断点只决定展开模式，永不决定按钮可用性。
import { defineStore } from "pinia";
import {
  loadSidebarPrefs,
  saveSidebarPrefs,
  SIDEBAR_MAX,
  SIDEBAR_MIN,
} from "../lib/sidebar-preference";
import { BREAKPOINT_MD, BREAKPOINT_SM } from "../lib/breakpoints";
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
    /** 用户手动折叠位（§3.40②·4：持久化；任何自动化永不改写——收窄/互斥
     * 折叠 ≠ 用户折叠）。展示态见 getter sidebarCollapsed。 */
    manualSidebarCollapsed: prefs.collapsed,
    /** 视口旗标（§3.40①，use-responsive-shell 写入；模块期先按当前窗口
     * 预置，挂载后校准——首帧无闪变）。overlay = 400–799 浮层互斥档。 */
    overlayMode:
      window.innerWidth >= BREAKPOINT_SM && window.innerWidth < BREAKPOINT_MD,
    tooNarrow: window.innerWidth < BREAKPOINT_SM,
    /** overlay 互斥的左栏瞬态折叠（§3.40②·5）：overlay 档展开右栏时挂上，
     * **不落盘**——回宽过 800（use-responsive-shell）或关闭右栏
     * （work-panel.setOpen）自动解除，「之前是自动折叠的」语义。 */
    overlaySidebarFold: false,
    /** 设置当前分区（§3.32 板面切换模型；重开设置页保持上次看过的板面）。 */
    settingsSection: "appearance" as SettingsSectionId,
    /** composer 聚焦令牌（§3.17：决策后焦点回输入框——批完立刻能接着
     * 打字）。PermissionCard 决策成功后自增，Composer watch 聚焦。 */
    composerFocusTick: 0,
    /** 命令面板开合（§3.15 v3.9：侧栏「搜索」行与 Ctrl+K 同流一实例）。 */
    paletteOpen: false,
  }),
  getters: {
    /** 展示态 = 手动折叠 ‖ overlay 互斥瞬态折叠（§3.40②·5）。 */
    sidebarCollapsed(state): boolean {
      return state.manualSidebarCollapsed || state.overlaySidebarFold;
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
     * 拖柄全走这里）。overlay 档互斥（§3.40②·5 方向不对称）：展开左栏 →
     * 右栏真实关闭 + 开意图消费（持久化 false，回宽不弹回）；被右栏排他
     * 折叠时的展开请求 = 换边（解瞬态旗标 + 收右栏）。 */
    toggleSidebar() {
      if (this.overlaySidebarFold) {
        this.overlaySidebarFold = false;
        if (window.innerWidth < BREAKPOINT_MD) useWorkPanelStore().setOpen(false);
        return;
      }
      this.manualSidebarCollapsed = !this.manualSidebarCollapsed;
      if (!this.manualSidebarCollapsed && window.innerWidth < BREAKPOINT_MD) {
        useWorkPanelStore().setOpen(false);
      }
      this.commitSidebarWidth();
    },
    /** overlay 互斥瞬态旗标读写（唯一调用方 = work-panel.setOpen）。 */
    setPanelExclusiveFold(fold: boolean) {
      this.overlaySidebarFold = fold;
    },
    /** 命令面板（§3.15）：Ctrl+K 与侧栏「搜索」行共用同一开合态。 */
    openPalette() {
      this.paletteOpen = true;
    },
    closePalette() {
      this.paletteOpen = false;
    },
    /** 视口旗标落位（§3.40①模式表；width clamp 是纯偏好修正，与折叠无关；
     * 只在 push 档让位主区——overlay 不挤压主内容，浮层渲染时钳 2/3 视口）。 */
    applyViewportWidth(w: number) {
      this.tooNarrow = w < BREAKPOINT_SM;
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
