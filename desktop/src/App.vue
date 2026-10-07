<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "./stores/approvals";
import { useAsksStore } from "./stores/asks";
import { useChatStore } from "./stores/chat";
import { useNotificationsStore } from "./stores/notifications";
import { useSettingsStore } from "./stores/settings";
import { useShellStore } from "./stores/shell";
import { useWorkPanelStore } from "./stores/work-panel";
import WorkPanel from "./components/shell/WorkPanel.vue";
import ChatTranscript from "./components/ChatTranscript.vue";
import WelcomeHome from "./components/chat/WelcomeHome.vue";
import Composer from "./components/chat/Composer.vue";
import ContextProgressBar from "./components/ContextProgressBar.vue";
import MemoryPage from "./components/MemoryPage.vue";
import SchedulePage from "./components/schedule/SchedulePage.vue";
import PluginMarket from "./components/PluginMarket.vue";
import SettingsPage from "./components/settings/SettingsPage.vue";
import CommandPalette from "./components/shell/CommandPalette.vue";
import DestinationShell from "./components/shell/DestinationShell.vue";
import Sidebar from "./components/shell/Sidebar.vue";
import ToastStack from "./components/shell/ToastStack.vue";
import Topbar from "./components/shell/Topbar.vue";
import WindowControls from "./components/shell/WindowControls.vue";
import { useGlobalShortcuts } from "./components/shell/use-global-shortcuts";
import { useResponsiveShell } from "./components/shell/use-responsive-shell";
import { computed, ref } from "vue";

/**
 * 壳骨架（UI shell doc §2.1 区域地图 / §2.6 保活规则）：
 * - 目的地一律整页接管 ③④⑤；设置更极端——连侧栏一起接管（§3.32，
 *   导航轨 + 页头 + 正文列自成一壳）；
 * - 对话视图不卸载：切走时 display:none + inert 挂起，回来恢复滚动与
 *   运行态（hidden 会被 flex 类覆盖，故用内联 display）；
 * - 其他目的地按需挂载（v-if），离开即卸载；侧栏（非设置目的地）恒显；
 * - 全局层：⌘K 命令面板 / 窗口控制挂壳根（跨视图）。审批与 ask 选择卡
 *   是 composer 坞内的决策卡（§3.17/§3.39①，v4.11 迁坞；决策后即隐，
 *   对话流零气泡）。
 */
const { t } = useI18n();
const chat = useChatStore();
const approvals = useApprovalsStore();
const asks = useAsksStore();
const settings = useSettingsStore();
const shell = useShellStore();
const notifications = useNotificationsStore();
const workPanel = useWorkPanelStore();
/** composer 坞总高：列宽热区下沿（ConversationWidthHandles）的锚。 */
const composerH = ref(0);
/** 首页空态（§3.14 v3.9）：转录未渲染欢迎页；composer Teleport 进槽位。 */
const isHome = computed(() => chat.items.length === 0);
/** §3.40④ overlay 遮罩（v2.111）：overlay 档且有浮层开着（互斥保证至多
 * 一个）；设置目的地整窗接管、侧栏不渲染，不挂遮罩。 */
const overlayMask = computed(
  () =>
    shell.overlayMode &&
    shell.destination !== "settings" &&
    (!shell.sidebarCollapsed || workPanel.open),
);
/** §3.4 v4.18：上下文用量进度条显隐（context.usageBar，缺省开）。 */
const showUsageBar = computed(() => settings.view?.context.usageBar ?? true);
/** 遮罩点击收起当前浮层（面板优先——互斥下两者不会同开，顺序只是防御）。 */
function dismissOverlay() {
  if (workPanel.open) workPanel.setOpen(false);
  else shell.toggleSidebar();
}
/** 欢迎页槽位元素（WelcomeHome defineExpose）：Teleport 直接吃元素不走
 * 选择器——初次挂载整棵树还没插进 document，选择器必扑空；元素就绪前
 * Teleport 保持 disabled（composer 停在底部坞原位）。 */
const welcomeRef = ref<{ slotEl: HTMLElement | null } | null>(null);
const welcomeSlotEl = computed(() => welcomeRef.value?.slotEl ?? null);
// §3.25 落地表：Ctrl+B/J/N/,/[/]（⌘K 与 Ctrl+Shift+L 各自注册，不在此）
useGlobalShortcuts();
// §3.40（v2.109）：视口断点旗标源——只读窗口写 store，不碰持久化状态
useResponsiveShell();

onMounted(() => {
  chat.init();
  approvals.init();
  void asks.init();
  void settings.init();
  void notifications.init();
});
</script>

<template>
  <div class="isolate flex h-full bg-page text-ink-1">
    <!-- Toast 栈（§3.22）：全局一次挂载，右下角 -->
    <ToastStack />
    <!-- 设置目的地：独立壳模式，整窗接管（§3.32 导航轨 + 页头 + 正文列） -->
    <template v-if="shell.destination === 'settings'">
      <SettingsPage />
    </template>

    <template v-else>
      <Sidebar />

      <!-- 对话主区：目的地切换时挂起，回来即恢复滚动与在跑的会话。
           data-chat-root = 列宽覆盖变量与拖拽锁的挂点（§2.7） -->
      <div
        data-chat-root
        class="relative flex min-w-0 flex-1 flex-col"
        :style="{ display: shell.destination === 'chat' ? '' : 'none' }"
        :inert="shell.destination !== 'chat'"
      >
        <Topbar />
        <!-- §3.23 会话冷切换：重放期间顶栏下方 2px 不定段进度轨 -->
        <div
          v-if="chat.replaying"
          class="replay-track"
          role="progressbar"
          :aria-label="t('chat.replaying')"
        >
          <div class="replay-track__bar" />
        </div>
        <ContextProgressBar v-if="showUsageBar" :usage="chat.contextUsage" />
        <!-- 首页空态（§3.14 v3.9）= 欢迎页独立形态；composer（单实例）
             Teleport 进欢迎页槽位，会话态回底部坞——草稿跨形态保留 -->
        <ChatTranscript v-if="!isHome" :composer-bottom="composerH" />
        <WelcomeHome v-else ref="welcomeRef" />
        <Teleport :to="welcomeSlotEl ?? 'body'" :disabled="!isHome || !welcomeSlotEl">
          <Composer @height="composerH = $event" />
        </Teleport>
      </div>

      <!-- 工作面板（§2.5，v2.52）：主区右缘第二展示面，只对对话目的地有
           意义（§2.4 开关在会话顶栏右侧）；随 chat 区一起挂起/显示。
           开合滑动动画与侧栏统一（§6.6 ②，issue #2）。首页空态不渲染
           （v2.60 修复批：会话态开面板切回首屏必须收起——空态锁右栏）。
           §3.40④（v2.111）：任何宽度可开——push 档停靠挤压、overlay 档
           （400–799）fixed 浮层贴右缘（互斥由 setOpen 收口），右栏永不
           自动展开（②·3 非对称裁定） -->
      <Transition name="wp">
        <WorkPanel
          v-if="shell.destination === 'chat' && !isHome && workPanel.open"
        />
      </Transition>

      <!-- §3.40④ overlay 遮罩（400–799 档）：浮层以外区域点击收起当前
           浮层（v2.111）。互斥保证至多一个浮层开着；z-25 压对话区顶栏
           z-10、让浮层 z-26 / 右上钉窗钮 z-30 / 窗口控制车道 z-1000 -->
      <div
        v-if="overlayMask"
        class="overlay-mask"
        aria-hidden="true"
        @click="dismissOverlay()"
      />

      <!-- §3.40① v2.112：无过窄提示层——应用 minWidth 400 硬限位即是
           保证，「窗口已经限制了，还要警告什么」（v2.109 的 narrow-veil
           与 narrow.* 文案键退役） -->

      <!-- 非对话目的地：侧栏右侧铺满，按需挂载 -->
      <DestinationShell
        v-if="shell.destination === 'plugins'"
        :title="t('nav.plugins')"
      >
        <PluginMarket />
      </DestinationShell>
      <DestinationShell
        v-else-if="shell.destination === 'memory'"
        :title="t('nav.memory')"
      >
        <MemoryPage />
      </DestinationShell>
      <DestinationShell
        v-else-if="shell.destination === 'schedule'"
        :title="t('nav.schedule')"
      >
        <SchedulePage />
      </DestinationShell>
    </template>

    <WindowControls />
    <CommandPalette />
  </div>
</template>

<style scoped>
/* §6.6 ② 工作面板抽屉（v3.9 issue #2；弹簧收尾 = 前端标准 §二① 动效
 * 矩阵裁定）：宽度驱动滑开，开 base + --ease-spring、收 fast/ease-in 不
 * 回弹；!important 压过内联宽度。transition 只挂在 enter/leave-active：
 * 拖宽与视口变化不动画。 */
.wp-enter-active {
  transition:
    width var(--dur-base) var(--ease-spring),
    transform var(--dur-base) var(--ease-spring),
    opacity var(--dur-base) var(--ease-out);
}
.wp-leave-active {
  transition:
    width var(--dur-fast) var(--ease-in),
    transform var(--dur-fast) var(--ease-in),
    opacity var(--dur-fast) var(--ease-in);
}
.wp-enter-from,
.wp-leave-to {
  width: 0 !important;
  opacity: 0;
  /* §3.40③：宽度收缩同时内容向右滑出（视觉滑出而非挤压重排） */
  transform: translateX(100%);
}

/* §3.40④ overlay 遮罩（400–799 档，v2.111）：浮层以外整面捕获点击收起。
 * z-25：压对话区顶栏 z-10，让浮层 z-26、右上钉窗钮 z-30、窗口控制车道
 * z-1000——浮层开着时右上角面板钮永远可点（换边走互斥收口）。淡入 fast */
.overlay-mask {
  position: fixed;
  inset: 0;
  z-index: 25;
  background: var(--lw-scrim);
  animation: lw-mask-in var(--dur-fast) var(--ease-out);
}
@keyframes lw-mask-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}
@media (prefers-reduced-motion: reduce) {
  .overlay-mask {
    animation: none;
  }
}

/* §3.40① v2.112：窗口过窄提示层退役——minWidth 400 硬限位即是保证，
 * 不再有任何「过窄警告」（用户裁定原文落档 §3.40 节首） */

/* §3.23 会话冷切换进度轨：2px 高、28% 宽不定段 720ms 来回扫 */
.replay-track {
  position: relative;
  height: 2px;
  flex: 0 0 2px;
  overflow: hidden;
  background: transparent;
}
.replay-track__bar {
  position: absolute;
  top: 0;
  left: 0;
  width: 28%;
  height: 100%;
  border-radius: var(--lw-r-full);
  background: var(--lw-ink-3);
  animation: lw-indeterminate 720ms ease-in-out infinite alternate;
}
@keyframes lw-indeterminate {
  /* 位移相对 bar 自身宽（28% 轨宽）：-110% 全出左缘 → 400%（=轨宽 112%）全出右缘 */
  from {
    transform: translateX(-110%);
  }
  to {
    transform: translateX(400%);
  }
}
/* §6.6 五：reduced-motion 下运行指示整块不显示 */
@media (prefers-reduced-motion: reduce) {
  .replay-track {
    display: none;
  }
}
</style>
