<script setup lang="ts">
/**
 * 侧栏（UI shell doc §2.1 ①② / §2.3 v3.9 项目接管）：顶部动作区三行 =
 * 新建任务（Ctrl+N）/ 搜索（Ctrl+K 开 §3.15 命令面板——搜全部可操作项）/
 * 插件市场（原「扩展」上移改名，issue #5⑪）；会话树在 SessionGroups
 * （项目段接管对话管理，issue #4）；页脚 = 设置/记忆/定时/通知 + 版本号
 * （issue #12，设置图标 = 齿轮）；版本号 7 连点解锁开发者选项（§7-16②）；
 * 折叠/拖宽在 use-sidebar-resize（§2.3，v3.9 拖拽不触发折叠）。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getVersion } from "@tauri-apps/api/app";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useNotificationsStore } from "../../stores/notifications";
import { SIDEBAR_MAX, SIDEBAR_MIN } from "../../lib/sidebar-preference";
import { useSidebarResize } from "./use-sidebar-resize";
import { useDevUnlock } from "./use-dev-unlock";
import { useNotificationEntry } from "./use-notification-entry";
import { Icon } from "../../lib/icons";
import SessionGroups from "./SessionGroups.vue";
import NotificationPopover from "./NotificationPopover.vue";

const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const settings = useSettingsStore();
const notifications = useNotificationsStore();

const version = ref("");

// 通知入口（§2.4 第 5 图标 + §3.13 浮层）：开合与锚定在 composable
const {
  open: notifOpen,
  anchor: notifAnchor,
  setBtn: setNotifBtn,
  toggle: toggleNotif,
} = useNotificationEntry();

onMounted(async () => {
  version.value = (await getVersion().catch(() => "")) || "";
});

// ---- 顶部动作区（§2.3 v3.9，issue #5⑪）----
function newTask() {
  chat.newSession();
  shell.setDestination("chat");
}
function openSearch() {
  shell.openPalette();
}

// 版本号连点解锁开发者选项（§3.2，安卓式 7 连点）：逻辑在 use-dev-unlock
const { onTap: onVersionTap } = useDevUnlock();

// ---- 拖拽调宽（§2.3 v3.9）：交互在 use-sidebar-resize.ts，状态与持久化在 shell store ----
const {
  dragging,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  endDrag,
  onKeyDown,
  onDoubleClick,
} = useSidebarResize();

const isMarket = computed(() => shell.destination === "plugins");
</script>

<template>
  <aside
    class="sidebar relative flex h-full flex-col overflow-hidden bg-side"
    :class="{ 'is-collapsed': shell.sidebarCollapsed }"
    :style="{ '--lw-sidebar-w': `${shell.sidebarWidth}px` }"
    :inert="shell.sidebarCollapsed"
    aria-label="Locowrk"
  >
    <div class="flex h-full w-[var(--lw-sidebar-w)] flex-col">
      <!-- 侧栏头（46px 带左段）：品牌 + 折叠按钮，空白处拖拽窗口 -->
      <header class="flex h-[var(--lw-toolbar-h)] shrink-0 items-center gap-2 px-2">
        <span
          class="flex items-center gap-2 text-[13.5px] font-semibold tracking-[-0.02em]"
          data-tauri-drag-region
        >
          <Icon name="sparkle" :size="14" />
          <span data-tauri-drag-region>Locowrk</span>
        </span>
        <div class="h-full min-w-0 flex-1" data-tauri-drag-region />
        <button
          class="ctl"
          type="button"
          :title="t('sidebar.collapse')"
          :aria-label="t('sidebar.collapse')"
          @click="shell.toggleSidebar()"
        >
          <Icon name="sidebar" :size="14" aria-hidden="true" />
        </button>
      </header>

      <!-- 侧栏体：顶部动作区 + 项目段（v3.9 项目接管，issue #4） -->
      <div class="scroll-y min-h-0 flex-1 px-2">
        <nav class="flex flex-col gap-[2px] pb-1 pt-1" :aria-label="t('sidebar.actions')">
          <button
            type="button"
            class="action-row"
            @click="newTask"
          >
            <Icon name="plus" aria-hidden="true" />
            <span class="action-row__label">{{ t("sidebar.newTask") }}</span>
            <span class="action-row__keys">Ctrl+N</span>
          </button>
          <button
            type="button"
            class="action-row"
            @click="openSearch"
          >
            <Icon name="search" aria-hidden="true" />
            <span class="action-row__label">{{ t("sidebar.searchAll") }}</span>
            <span class="action-row__keys">Ctrl+K</span>
          </button>
          <button
            type="button"
            class="action-row"
            :aria-pressed="isMarket"
            @click="shell.setDestination('plugins')"
          >
            <Icon name="plug" aria-hidden="true" />
            <span class="action-row__label">{{ t("nav.plugins") }}</span>
          </button>
        </nav>

        <!-- 项目段（§2.3 v3.9）：组集合 = fsRoots ∪ 已关联会话，未归组 -->
        <SessionGroups :sessions="chat.sessions" :fs-roots="settings.view?.tools.fsRoots ?? []" />
      </div>

      <!-- 侧栏底（issue #12 序：设置/记忆/定时/通知）+ 版本号（连点解锁
           开发者选项）；间距对原型（gap 2px / padding 0 8px） -->
      <footer
        class="flex h-[var(--lw-toolbar-h)] shrink-0 items-center gap-[2px] px-2"
      >
        <button
          v-for="dest in (['settings', 'memory', 'schedule'] as const)"
          :key="dest"
          class="ctl"
          type="button"
          :aria-pressed="shell.destination === dest"
          :title="t(`nav.${dest}`)"
          :aria-label="t(`nav.${dest}`)"
          @click="shell.setDestination(dest)"
        >
          <Icon
            :name="dest === 'memory' ? 'database' : dest === 'schedule' ? 'clock' : 'settings'"
            :size="15"
            aria-hidden="true"
          />
        </button>
        <!-- 通知（§2.4 第 5 图标，popover 非页面 §3.13）；未读 6px accent 点 -->
        <button
          :ref="setNotifBtn"
          class="ctl notif-btn"
          type="button"
          :aria-pressed="notifOpen"
          :title="t('nav.notifications')"
          :aria-label="t('nav.notifications')"
          @click="toggleNotif"
        >
          <Icon name="bell" :size="15" aria-hidden="true" />
          <span
            v-if="notifications.unread > 0"
            class="notif-badge"
            aria-hidden="true"
          />
        </button>
        <button
          v-if="version"
          type="button"
          class="version-label ml-auto text-[11px] tabular-nums text-ink-4"
          :title="
            settings.view?.developer.unlocked ? t('nav.settings') : t('sidebar.devUnlockHint')
          "
          @click="onVersionTap"
        >
          v{{ version }}
        </button>
      </footer>
    </div>

    <!-- 通知浮层（§3.13）：fixed 锚触发钮上方；Teleport 出 aside 裁剪 +
         Transition 接管进退场（标准 §二① 禁止无退场） -->
    <Teleport to="body">
      <Transition name="lw-pop-up">
        <NotificationPopover
          v-if="notifOpen"
          :x="notifAnchor.x"
          :y="notifAnchor.y"
          @close="notifOpen = false"
        />
      </Transition>
    </Teleport>

    <!-- 拖拽柄（§2.3 v3.9）：8px 热区嵌右缘，可见提示 = 2×32 圆角条 -->
    <button
      ref="grip"
      class="sidebar-grip"
      type="button"
      role="separator"
      aria-orientation="vertical"
      :aria-label="t('sidebar.resize')"
      :aria-valuemin="SIDEBAR_MIN"
      :aria-valuemax="SIDEBAR_MAX"
      :aria-valuenow="shell.sidebarWidth"
      :aria-valuetext="t('sidebar.resizeText', { n: shell.sidebarWidth })"
      :tabindex="shell.sidebarCollapsed ? -1 : 0"
      :data-dragging="dragging ? '' : undefined"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="endDrag(false)"
      @lostpointercapture="endDrag(true)"
      @keydown="onKeyDown"
      @dblclick="onDoubleClick"
    />
  </aside>
</template>

<style scoped>
/* 侧栏宽 = 拖拽实时值（JS 写 --lw-sidebar-w）；折叠 = 独立状态宽 0、
 * 体不卸载；展开 base + --ease-spring（前端标准 §二① 侧滑弹簧收尾），
 * 收起 fast/ease-in 不回弹（§6.6 ②③抽屉） */
.sidebar {
  flex: 0 0 var(--lw-sidebar-w);
  transition:
    flex-basis var(--dur-base) var(--ease-spring),
    opacity var(--dur-base) var(--ease-out);
}
.sidebar.is-collapsed {
  flex-basis: 0;
  opacity: 0;
  pointer-events: none;
  transition:
    flex-basis var(--dur-fast) var(--ease-in),
    opacity var(--dur-fast) var(--ease-in);
}

/* 28px 方形图标控件（§3.19） */
.ctl {
  display: inline-flex;
  width: var(--lw-ctl);
  height: var(--lw-ctl);
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-md);
  color: var(--lw-ink-2);
}
.ctl:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}

/* 顶部动作区行（§2.3 v3.9）：30px = 15px 字形 + 13px 文案 + 行尾键帽 */
.action-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 30px;
  padding: 0 8px;
  border-radius: var(--lw-r-xs);
  color: var(--lw-ink-2);
  text-align: left;
}
.action-row:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.action-row[aria-pressed="true"] {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}
.action-row__label {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}
.action-row__keys {
  flex: 0 0 auto;
  font-size: 10.5px;
  color: var(--lw-ink-4);
}

/* 通知钮未读角标（§3.13）：6px accent 点 */
.notif-btn {
  position: relative;
}
.notif-badge {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 6px;
  height: 6px;
  border-radius: var(--lw-r-full);
  background: var(--lw-accent);
}

/* 版本号（连点解锁开发者选项的锚点） */
.version-label {
  border-radius: var(--lw-r-xs);
  padding: 2px 6px;
}
.version-label:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-2);
}

/* 拖拽柄（§2.3 v3.9）：8px 热区；提示 2×32 圆角条 hover/聚焦/拖拽显形，
 * 聚焦与拖拽换 accent，聚焦不画外框 */
.sidebar-grip {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 8px;
  cursor: col-resize;
  touch-action: none;
  user-select: none;
}
.sidebar-grip::after {
  content: "";
  position: absolute;
  top: 50%;
  right: 2px;
  width: 2px;
  height: 32px;
  border-radius: var(--lw-r-full);
  background: color-mix(in oklab, var(--lw-ink-1) 20%, transparent);
  opacity: 0;
  transform: translateY(-50%);
  transition: opacity var(--dur-fast) var(--ease-out);
}
.sidebar-grip:hover::after,
.sidebar-grip:focus-visible::after,
.sidebar-grip[data-dragging]::after {
  opacity: 1;
}
.sidebar-grip:focus-visible {
  outline: none;
}
.sidebar-grip:focus-visible::after {
  background: var(--lw-accent);
}
</style>
