<script setup lang="ts">
/**
 * 侧栏（UI shell doc §2.1 ①② / §2.3 / §3.6）：会话运营 + 目的地入口。
 * - 一级分组 = 「会话」（会话↔工作区关联 §8.2 未落地前全量落这里）；
 *   二级分组 = 时间桶（近 7 天 / 近 14 天 / 更早）。
 * - 搜索框 = 标题前端过滤 + 正文命中（v2.49 `session_search`，命中行带
 *   摘录，mtime 降序限量）。
 * - 行渲染与运营（重命名/导出/删除）在 SessionRow（§3.6 行尾 `…` 菜单）。
 * - 版本号连点 7 次 = 解锁「开发者」设置分区（安卓式，可配置原则
 *   §7-16②；解锁态存 settings.json `developer.unlocked`）。
 * - 折叠/拖宽交互在 use-sidebar-resize（§2.3）；宽度跨重启持久化。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getVersion } from "@tauri-apps/api/app";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import type { SessionMeta } from "../../types";
import { SIDEBAR_MAX, SIDEBAR_MIN } from "../../lib/sidebar-preference";
import { useSidebarResize } from "./use-sidebar-resize";
import { useSessionSearch } from "./use-session-search";
import { Icon } from "../../lib/icons";
import SessionRow from "./SessionRow.vue";

const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const settings = useSettingsStore();
const toast = useToastStore();

const search = ref("");
const version = ref("");

onMounted(async () => {
  version.value = (await getVersion().catch(() => "")) || "";
});

// ---- 时间桶（§2.3 二级分组：按最后活动时间，前端即可算） ----
const DAY = 86_400_000;
const BUCKETS = [
  { key: "bucket7", limit: 7 * DAY },
  { key: "bucket14", limit: 14 * DAY },
  { key: "earlier", limit: Number.POSITIVE_INFINITY },
] as const;

function bucketOf(tsSeconds: number): (typeof BUCKETS)[number] {
  const age = Date.now() - tsSeconds * 1000;
  return BUCKETS.find((b) => age <= b.limit) ?? BUCKETS[2];
}

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return chat.sessions;
  return chat.sessions.filter((s) =>
    (s.title ?? s.id).toLowerCase().includes(q),
  );
});

const grouped = computed(() => {
  const out: { key: string; items: SessionMeta[] }[] = BUCKETS.map((b) => ({
    key: b.key,
    items: [],
  }));
  for (const s of filtered.value) {
    const idx = BUCKETS.indexOf(bucketOf(s.updatedAt));
    out[idx === -1 ? 2 : idx].items.push(s);
  }
  return out.filter((g) => g.items.length > 0);
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

// ---- 版本号连点解锁开发者选项（§3.2，安卓式 7 连点）----
let devTaps = 0;
let lastTapAt = 0;

function onVersionTap() {
  if (settings.view?.developer.unlocked) return;
  const now = Date.now();
  if (now - lastTapAt > 2000) devTaps = 0;
  lastTapAt = now;
  devTaps += 1;
  if (devTaps >= 7) {
    devTaps = 0;
    void settings.saveDeveloper({ unlocked: true });
    toast.push("ok", t("sidebar.devUnlocked"));
  } else if (devTaps >= 3) {
    toast.push(
      "ok",
      t("sidebar.devUnlockProgress", { n: 7 - devTaps }),
      undefined,
      "dev-unlock",
    );
  }
}

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

        <div class="side-label">{{ t("sidebar.sessions") }}</div>

        <template v-for="group in grouped" :key="group.key">
          <div class="bucket-label">{{ t(`sidebar.${group.key}`) }}</div>
          <SessionRow
            v-for="s in group.items"
            :key="s.id"
            :session="s"
          />
        </template>

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
/* 侧栏宽 = 拖拽后的实时值（JS 写 --lw-sidebar-w）；折叠 = 独立状态宽 0，
 * 体不卸载（内层固定宽保滚动位置与不回流）。展开 --dur-base + ease-out，
 * 收起 --dur-fast + ease-in（§6.6 ②③抽屉）。 */
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

.side-label {
  padding: 10px 8px 4px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
.bucket-label {
  padding: 6px 8px 2px 28px;
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

/* 拖拽柄（§2.3）：8px 热区；可见提示 2×32 圆角条，hover/聚焦/拖拽显形，
 * 聚焦与拖拽换 accent，聚焦不画外框（圆角条即焦点提示） */
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
