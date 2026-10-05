<script setup lang="ts">
/**
 * 命令面板（UI shell doc §3.15）：Ctrl/⌘+K 打开，唯一的「跳任何地方」入口；
 * v3.9 起侧栏「搜索」行与本面板同流（开合态在 shell store）。分组 = 会话 /
 * 正文命中（v3.9 自侧栏搜索框并入，§3.6）/ 页面 / 设置 / 命令，形态照原型：
 * 52px 搜索行 + Esc 键帽、结果 max-h 滚动、11px 组标签、12.5px 行 + 行尾键帽
 * 或所属分区；当前行 wash-2 高亮，↑↓ 移动、Enter 执行、Esc 关闭、鼠标跟随；
 * 选中「设置」类结果 → shell.openSettings。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useShellStore } from "../../stores/shell";
import { useSettingsStore } from "../../stores/settings";
import { useWorkPanelStore } from "../../stores/work-panel";
import { relativeTime } from "../../lib/format";
import { useSessionSearch } from "./use-session-search";
import {
  SETTINGS_SECTIONS,
  isSectionVisible,
} from "../../lib/settings-index";
import SearchInput from "../patterns/SearchInput.vue";
import { flipEffectiveTheme } from "../../lib/theme";

const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const settingsStore = useSettingsStore();

const query = ref("");
const searchInput = ref<InstanceType<typeof SearchInput> | null>(null);
const resultsEl = ref<HTMLElement | null>(null);
const highlight = ref(0);
// 正文命中（v2.49 能力随侧栏搜索框并入，§3.15 v3.9）
const { contentHits } = useSessionSearch(query);

interface PaletteItem {
  key: string;
  label: string;
  hint?: string;
  /** 键帽（§3.25：每个快捷键都能在面板里按名字搜到、行尾挂键帽）。 */
  keys?: string;
  run: () => void;
}
interface PaletteGroup {
  labelKey: string;
  items: PaletteItem[];
}

/** 全量项；过滤在 computed 里做（索引驱动，不爬 DOM）。 */
const allGroups = computed<PaletteGroup[]>(() => {
  const sessions: PaletteItem[] = chat.sessions.map((s) => {
    const rt = relativeTime(s.updatedAt);
    return {
      key: `session:${s.id}`,
      label: s.title ?? t("sidebar.untitled"),
      hint: rt.n !== undefined ? t(`memory.time.${rt.key}`, { n: rt.n }) : t(`memory.time.${rt.key}`),
      run: () => {
        if (chat.running && s.id !== chat.sessionId) return; // 运行中禁切
        void chat.switchSession(s.id);
        shell.setDestination("chat");
      },
    };
  });
  const bodyHits: PaletteItem[] = contentHits.value.map((h) => ({
    key: `body:${h.id}`,
    label: h.title ?? t("sidebar.untitled"),
    hint: h.snippet,
    run: () => {
      if (chat.running && h.id !== chat.sessionId) return;
      void chat.switchSession(h.id);
      shell.setDestination("chat");
    },
  }));
  const pages: PaletteItem[] = (
    [
      ["chat", "nav.chat"],
      ["plugins", "nav.plugins"],
      ["memory", "nav.memory"],
      ["schedule", "nav.schedule"],
      ["settings", "nav.settings"],
    ] as const
  ).map(([dest, key]) => ({
    key: `page:${dest}`,
    label: t(key),
    run: () => shell.setDestination(dest),
  }));
  const devUnlocked = settingsStore.view?.developer.unlocked ?? false;
  const settings: PaletteItem[] = SETTINGS_SECTIONS.filter((s) =>
    isSectionVisible(s, devUnlocked),
  ).map((s) => ({
    key: `settings:${s.id}`,
    label: t(s.labelKey),
    hint: `${t("nav.settings")} / ${t(s.labelKey)}`,
    run: () => shell.openSettings(s.id),
  }));
  /** 明暗主题切换：实效档解析在 lib/theme（flipEffectiveTheme）。 */
  function toggleTheme() {
    void settingsStore.setTheme(
      flipEffectiveTheme(settingsStore.view?.appearance.theme ?? "light"),
    );
  }

  const commands: PaletteItem[] = [
    {
      key: "cmd:newTask",
      label: t("sidebar.newTask"),
      keys: "Ctrl N",
      run: () => {
        chat.newSession();
        shell.setDestination("chat");
      },
    },
    {
      key: "cmd:toggleSidebar",
      label: t("palette.cmd.toggleSidebar"),
      keys: "Ctrl B",
      run: () => shell.toggleSidebar(),
    },
    {
      key: "cmd:togglePanel",
      label: t("palette.cmd.togglePanel"),
      keys: "Ctrl J",
      run: () => {
        shell.setDestination("chat");
        // §3.14 v3.9（issue #10）：首页空态不可展开工作面板
        if (chat.items.length > 0) useWorkPanelStore().toggle();
      },
    },
    {
      key: "cmd:openSettings",
      label: t("palette.cmd.openSettings"),
      keys: "Ctrl ,",
      run: () => shell.openSettings(),
    },
    {
      key: "cmd:toggleTheme",
      label: t("palette.cmd.toggleTheme"),
      run: toggleTheme,
    },
  ];
  const groups: PaletteGroup[] = [
    { labelKey: "palette.groups.sessions", items: sessions },
  ];
  if (query.value.trim() && bodyHits.length > 0)
    groups.push({ labelKey: "palette.groups.contentHits", items: bodyHits });
  groups.push(
    { labelKey: "palette.groups.pages", items: pages },
    { labelKey: "palette.groups.settings", items: settings },
    { labelKey: "palette.groups.commands", items: commands },
  );
  return groups;
});

const visibleGroups = computed<PaletteGroup[]>(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return allGroups.value;
  return allGroups.value
    .map((g) => ({
      labelKey: g.labelKey,
      items: g.items.filter((it) => it.label.toLowerCase().includes(q)),
    }))
    .filter((g) => g.items.length > 0);
});

const flatItems = computed(() => visibleGroups.value.flatMap((g) => g.items));

/** 组内第 i 项在扁平列表里的下标（键盘高亮跨组连续）。 */
function globalIndex(group: PaletteGroup, i: number): number {
  const gi = visibleGroups.value.indexOf(group);
  return (
    visibleGroups.value.slice(0, gi).reduce((n, g) => n + g.items.length, 0) + i
  );
}

// 过滤结果变化 → 高亮回第一项（原型同款语义）
watch(flatItems, () => {
  highlight.value = 0;
});

function moveHighlight(delta: number) {
  const n = flatItems.value.length;
  if (!n) return;
  highlight.value = (highlight.value + delta + n) % n;
  void nextTick(() => {
    resultsEl.value
      ?.querySelectorAll("[data-palette-item]")
      [highlight.value]?.scrollIntoView({ block: "nearest" });
  });
}

function activate(item: PaletteItem) {
  close();
  item.run();
}

function onItemEnter(index: number) {
  highlight.value = index;
}

function openPalette() {
  query.value = "";
  highlight.value = 0;
  shell.openPalette();
  void nextTick(() => searchInput.value?.focus());
}

function close() {
  shell.closePalette();
}

// 开合态在 shell store（§3.15 v3.9）：侧栏「搜索」行置位时在此就绪
watch(
  () => shell.paletteOpen,
  (v) => {
    if (v) {
      query.value = "";
      highlight.value = 0;
      void nextTick(() => searchInput.value?.focus());
    }
  },
);

function onGlobalKeydown(e: KeyboardEvent) {
  const mod = e.ctrlKey || e.metaKey;
  if (mod && e.key.toLowerCase() === "k") {
    e.preventDefault();
    if (shell.paletteOpen) close();
    else openPalette();
    return;
  }
  if (!shell.paletteOpen) return;
  if (e.key === "Escape") {
    e.preventDefault();
    close();
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    moveHighlight(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveHighlight(-1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    const item = flatItems.value[highlight.value];
    if (item) activate(item);
  }
}

onMounted(() => document.addEventListener("keydown", onGlobalKeydown));
onBeforeUnmount(() => document.removeEventListener("keydown", onGlobalKeydown));
</script>

<template>
  <Transition name="lw-modal">
    <!-- ⌘K = 居中偏上弹出 + 统一遮罩（用户 2026-10-06 裁定，标准 §二①）；
         14vh + 面板最高 60vh ⇒ 视觉重心 ≈44vh（居中偏上）；z = 模态档 -->
    <div
      v-if="shell.paletteOpen"
      class="fixed inset-0 z-[var(--lw-z-modal)] flex items-start justify-center px-6"
      style="background: var(--lw-scrim); padding-top: 14vh"
      @mousedown.self="close"
    >
      <div
        class="palette lw-modal-panel"
        role="dialog"
        aria-modal="true"
        :aria-label="t('palette.title')"
      >
        <div class="palette-search">
          <SearchInput
            ref="searchInput"
            v-model="query"
            :placeholder="t('palette.placeholder')"
            :label="t('palette.placeholder')"
          />
          <span class="keycap">Esc</span>
        </div>
        <div ref="resultsEl" class="palette-results scroll-y" role="listbox">
          <template
            v-for="(group, gi) in visibleGroups"
            :key="group.labelKey"
          >
            <div
              class="palette-group-label"
              :class="{ 'palette-group-label--ruled': gi > 0 }"
            >
              {{ t(group.labelKey) }}
            </div>
            <button
              v-for="(item, i) in group.items"
              :key="item.key"
              class="palette-item"
              type="button"
              role="option"
              data-palette-item
              :aria-selected="highlight === globalIndex(group, i) ? 'true' : 'false'"
              @click="activate(item)"
              @mouseenter="onItemEnter(globalIndex(group, i))"
            >
              <span class="truncate">{{ item.label }}</span>
              <span v-if="item.keys" class="keycap">{{ item.keys }}</span>
              <span v-else-if="item.hint" class="palette-item-hint">
                {{ item.hint }}
              </span>
            </button>
          </template>
          <p
            v-if="flatItems.length === 0"
            class="px-2.5 py-3 text-xs text-ink-4"
          >
            {{ t("palette.empty") }}
          </p>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.palette {
  display: flex;
  /* 中上（标准 §二①②）：顶缘 14vh + 面板最高 56vh ⇒ 视觉重心 ≈42vh；
   * 上限同时按视口比例封顶，矮窗口（480px 上限时）重心不会被拖回中间 */
  max-height: min(56vh, 440px);
  width: min(100%, 640px);
  flex-direction: column;
  overflow: hidden;
  border-radius: var(--lw-r-lg-plus);
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-dialog);
}
.palette-search {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-10);
  height: 52px;
  flex-shrink: 0;
  padding: 0 var(--lw-gap-14);
  border-bottom: 1px solid var(--lw-line-subtle);
}
.palette-results {
  max-height: min(52vh, 420px);
  padding: var(--lw-gap-6);
}
.palette-group-label {
  padding: var(--lw-gap-8) var(--lw-gap-10) var(--lw-gap-4);
  font-size: var(--lw-fs-11);
  letter-spacing: 0.02em;
  color: var(--lw-ink-4);
}
/* v3.9 用户裁定：分组间淡发丝线，两端不到头（inset 12px） */
.palette-group-label--ruled {
  border-top: 1px solid var(--lw-line-subtle);
  margin: var(--lw-gap-4) var(--lw-gap-12) 0;
  padding: var(--lw-gap-8) 0 var(--lw-gap-4);
}
.palette-item {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  width: 100%;
  border-radius: var(--lw-r-xs);
  padding: 7px var(--lw-gap-8); /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  font-size: var(--lw-fs-12half);
  line-height: 1.3;
  color: var(--lw-ink-1);
  text-align: left;
  transition: background var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}
.palette-item[aria-selected="true"] {
  background: var(--lw-wash-2);
}
.palette-item-hint {
  margin-left: auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 1;
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
.keycap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 18px;
  flex-shrink: 0;
  padding: 0 5px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值5,§二⑬) */
  border-radius: var(--lw-r-3xs);
  background: var(--lw-wash-2);
  font-size: var(--lw-fs-11);
  font-weight: 520;
  color: var(--lw-ink-3);
}
</style>
