<script setup lang="ts">
/**
 * 侧栏（UI shell doc §2.1 ①② / §2.3 / §3.6）：会话树在 SessionGroups
 * （§2.3 两级分组）；搜索 = 标题过滤 + 正文命中（v2.49）；通知 = 第 5
 * 图标 + §3.13 浮层（v2.51）；版本号 7 连点解锁开发者选项（§7-16②）；
 * 折叠/拖宽在 use-sidebar-resize（§2.3），宽度跨重启持久化。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getVersion } from "@tauri-apps/api/app";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useNotificationsStore } from "../../stores/notifications";import { SIDEBAR_MAX, SIDEBAR_MIN } from "../../lib/sidebar-preference";
import { useSidebarResize } from "./use-sidebar-resize";
import { useDevUnlock } from "./use-dev-unlock";
import { useNotificationEntry } from "./use-notification-entry";
import { useSessionSearch } from "./use-session-search";
import { Icon } from "../../lib/icons";
import SessionGroups from "./SessionGroups.vue";
import NotificationPopover from "./NotificationPopover.vue";

const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const settings = useSettingsStore();
const notifications = useNotificationsStore();

const search = ref("");
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

// ---- 分组树（§2.3）：两级分组在 lib/session-groups.ts + SessionGroups ----
const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return chat.sessions;
  return chat.sessions.filter((s) =>
    (s.title ?? s.id).toLowerCase().includes(q),
  );
});

// ---- 正文搜索（v2.49）：防抖/竞态守卫在 use-session-search.ts ----
const { contentHits } = useSessionSearch(search);

const nothingFound = computed(
  () => filtered.value.length === 0 && contentHits.value.length === 0,
);

// ---- 会话导航（§3.6）：正文命中行也走这里 ----
function openSession(id: string) {
  if (chat.running && id !== chat.sessionId) return; // 运行中禁切（§3.6）
  void chat.switchSession(id);
  shell.setDestination("chat");
}

// 版本号连点解锁开发者选项（§3.2，安卓式 7 连点）：逻辑在 use-dev-unlock
const { onTap: onVersionTap } = useDevUnlock();

// ---- 拖拽调宽（§2.3）：交互在 use-sidebar-resize.ts，状态与持久化在 shell store ----
const {
  dragging,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  endDrag,
  onKeyDown,
  onDoubleClick,
} = useSidebarResize();
</script>

<template>
  <aside
    class="sidebar relative flex h-full flex-col overflow-hidden bg-side"
    :class="{ 'is-collapsed': shell.sidebarCollapsed }"
    :inert="shell.sidebarCollapsed"
    aria-label="Locowrk"
  >
    <div class="flex h-full w-[var(--lw-sidebar-w)] flex-col">
      <!-- 侧栏头（46px 带左段）：品牌 + 折叠按钮，空白处拖拽窗口 -->
      <header class="flex h-[var(--lw-toolbar-h)] shrink-0 items-center gap-2 pl-4 pr-2">
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

      <!-- 侧栏体：搜索 / 新会话 + 会话（时间桶 / 正文命中） -->
      <div class="scroll-y min-h-0 flex-1 px-2">
        <div class="flex items-center gap-1.5 px-1 pb-2 pt-1">
          <div class="relative min-w-0 flex-1">
            <Icon
              name="search"
              :size="14"
              class="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-ink-4"
            />
            <input
              v-model="search"
              type="text"
              :placeholder="t('sidebar.search')"
              :aria-label="t('sidebar.search')"
              class="h-7 w-full rounded-full bg-wash-2 pl-8 pr-2 text-[12.5px] text-ink-1 outline-none placeholder:text-ink-5"
            />
          </div>
          <button
            class="ctl"
            type="button"
            :title="t('sidebar.newChat')"
            :aria-label="t('sidebar.newChat')"
            @click="chat.newSession(); shell.setDestination('chat')"
          >
            <Icon name="plus" aria-hidden="true" />
          </button>
        </div>

        <!-- 会话树（§2.3）：「会话」组 + 「项目」组（工作区→时间桶） -->
        <SessionGroups :sessions="filtered" />

        <!-- 正文命中（v2.49）：标题过滤之后的第二段 -->
        <template v-if="search.trim() && contentHits.length > 0">
          <div class="side-label">{{ t("sidebar.contentHits") }}</div>
          <button
            v-for="h in contentHits"
            :key="h.id"
            type="button"
            class="hit-row"
            :data-selected="h.id === chat.sessionId || undefined"
            :title="h.snippet"
            @click="openSession(h.id)"
          >
            <span class="hit-row__title">{{ h.title ?? t("sidebar.untitled") }}</span>
            <span class="hit-row__snippet">{{ h.snippet }}</span>
          </button>
        </template>

        <p v-if="nothingFound" class="px-2 py-4 text-center text-[12px] text-ink-4">
          {{ t("sidebar.searchEmpty") }}
        </p>
      </div>

      <!-- 侧栏底：目的地入口 + 版本号（连点解锁开发者选项） -->
      <footer
        class="flex h-[var(--lw-toolbar-h)] shrink-0 items-center gap-1 px-3"
      >
        <button
          v-for="dest in (['plugins', 'memory', 'settings'] as const)"
          :key="dest"
          class="ctl"
          type="button"
          :aria-pressed="shell.destination === dest"
          :title="t(`nav.${dest}`)"
          :aria-label="t(`nav.${dest}`)"
          @click="shell.setDestination(dest)"
        >
          <Icon
            :name="dest === 'plugins' ? 'plug' : dest === 'memory' ? 'database' : 'settings'"
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

    <!-- 通知浮层（§3.13）：fixed 锚触发钮上方，不受 aside overflow 裁剪 -->
    <NotificationPopover
      v-if="notifOpen"
      :x="notifAnchor.x"
      :y="notifAnchor.y"
      @close="notifOpen = false"
    />

    <!-- 拖拽柄（§2.3）：8px 热区嵌右缘，可见提示 = 2×32 圆角条 -->
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
 * 体不卸载；展开 base/ease-out，收起 fast/ease-in（§6.6 ②③抽屉） */
.sidebar {
  flex: 0 0 var(--lw-sidebar-w);
  transition:
    flex-basis var(--dur-base) var(--ease-out),
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

.side-label {
  padding: 10px 8px 4px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}

/* 正文命中行：标题 + 摘录两行（摘录 11.5px ink-4 单行省略） */
.hit-row {
  display: flex;
  min-height: 28px;
  width: 100%;
  flex-direction: column;
  justify-content: center;
  gap: 1px;
  border-radius: var(--lw-r-xs);
  padding: 2px 8px 2px 28px;
  text-align: left;
}
.hit-row:hover {
  background: var(--lw-wash-2);
}
.hit-row[data-selected] {
  background: var(--lw-wash-3);
}
.hit-row__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  color: var(--lw-ink-2);
}
.hit-row[data-selected] .hit-row__title {
  color: var(--lw-ink-1);
}
.hit-row__snippet {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11.5px;
  color: var(--lw-ink-4);
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

/* 拖拽柄（§2.3）：8px 热区；提示 2×32 圆角条 hover/聚焦/拖拽显形，
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
