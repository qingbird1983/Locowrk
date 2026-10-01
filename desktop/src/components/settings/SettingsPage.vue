<script setup lang="ts">
/**
 * 设置整页组装（UI shell doc §3.31/§3.32）：独立壳模式——导航轨（275px，
 * 左）+ 内容区（56px 页头显示当前分区 + 760 正文列滚动）。当前分区由
 * 滚动侦测驱动（标题越线即换高亮）；导航轨点击与 ⌘K 命中共用
 * scrollToSection，⌘K 命中额外闪一下（锚点高亮，§3.15）。
 */
import { nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useShellStore } from "../../stores/shell";
import {
  SETTINGS_SECTIONS,
  type SettingsSectionId,
} from "../../lib/settings-index";
import SettingsNav from "../shell/SettingsNav.vue";
import SettingsPanel from "../SettingsPanel.vue";

const { t } = useI18n();
const shell = useShellStore();

const scroller = ref<HTMLElement | null>(null);
const current = ref<SettingsSectionId>(SETTINGS_SECTIONS[0]!.id);

function sectionLabel(id: SettingsSectionId): string {
  const s = SETTINGS_SECTIONS.find((x) => x.id === id);
  return s ? t(s.labelKey) : id;
}

/** 滚到分区（导航轨点击 / ⌘K 命中共用）；flash = 锚点高亮闪一下。 */
function scrollToSection(id: SettingsSectionId, flash: boolean) {
  const sc = scroller.value;
  const el = sc?.querySelector<HTMLElement>(
    `[data-settings-section="${id}"]`,
  );
  if (!sc || !el) return;
  sc.scrollTo({ top: Math.max(0, el.offsetTop - 12), behavior: "smooth" });
  if (flash) {
    el.classList.remove("settings-flash");
    void el.offsetWidth; // 重触发：同分区连跳两次也能再闪
    el.classList.add("settings-flash");
  }
}

function onSelect(id: SettingsSectionId) {
  current.value = id;
  scrollToSection(id, false);
}

/** 滚动侦测：分区标题越线即换当前分区（导航轨高亮跟随内容）。 */
function onScroll() {
  const sc = scroller.value;
  if (!sc) return;
  const top = sc.getBoundingClientRect().top;
  let id = SETTINGS_SECTIONS[0]!.id;
  for (const el of sc.querySelectorAll<HTMLElement>(
    "[data-settings-section]",
  )) {
    if (el.getBoundingClientRect().top - top <= 88) {
      id = (el.dataset.settingsSection ?? id) as SettingsSectionId;
    }
  }
  current.value = id;
}

// ⌘K 命中（§3.32 三）：已在设置页 → 直接滚；从别的目的地跳进来 →
// settingsFocus 在挂载前已置好，onMounted 补一次。
watch(
  () => shell.settingsFocus,
  (focus) => {
    if (!focus) return;
    current.value = focus.section;
    void nextTick(() => scrollToSection(focus.section, true));
  },
);

onMounted(() => {
  onScroll();
  const focus = shell.settingsFocus;
  if (focus) {
    current.value = focus.section;
    void nextTick(() => scrollToSection(focus.section, true));
  }
});
</script>

<template>
  <div class="flex h-full min-w-0 flex-1">
    <SettingsNav :current="current" @select="onSelect" />
    <div class="flex min-w-0 flex-1 flex-col bg-page">
      <!-- 56px 目的地页头（§3.31）：当前分区标题 20px/600/-0.02em -->
      <header
        class="flex h-14 shrink-0 items-center gap-3 px-6"
        style="padding-right: var(--lw-wincontrols)"
      >
        <h1
          class="truncate text-[20px] font-semibold tracking-[-0.02em] text-ink-1"
        >
          {{ sectionLabel(current) }}
        </h1>
        <div class="h-full min-w-0 flex-1" data-tauri-drag-region />
      </header>
      <div
        ref="scroller"
        class="scroll-y relative min-h-0 flex-1"
        @scroll.passive="onScroll"
      >
        <SettingsPanel />
      </div>
    </div>
  </div>
</template>
