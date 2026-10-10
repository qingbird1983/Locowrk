<script setup lang="ts">
/**
 * 设置整页组装（UI shell doc §3.31/§3.32 v3.4 板面切换模型）：独立壳模式
 * ——导航轨（275px，左）+ 内容区（56px 页头 + 单板面）。右栏同一时刻只
 * 渲染 shell.settingsSection 指向的分区组件：不堆叠、不滚动定位、页头标题
 * 跟随当前分区。轨点击与 ⌘K 命中都改写同一份 store 状态，组件纯响应式。
 * "settings.json 分区 ↔ 面板分区一一对应"由本文件的 Record 映射机械保证
 * （索引里有而这里没组件 = 构建期类型错误）。
 */
import { computed, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { useShellStore } from "../../stores/shell";
import { useSettingsStore } from "../../stores/settings";
import {
  SETTINGS_SECTIONS,
  isSectionVisible,
  type SettingsSectionId,
} from "../../lib/settings-index";
import SettingsNav from "../shell/SettingsNav.vue";
import AppearanceSection from "./AppearanceSection.vue";
import ChatSection from "./ChatSection.vue";
import ContextSection from "./ContextSection.vue";
import DeveloperSection from "./DeveloperSection.vue";
import ToolsSection from "./ToolsSection.vue";
import EmbeddingSection from "./EmbeddingSection.vue";
import LanguageSection from "./LanguageSection.vue";
import LlmSection from "./LlmSection.vue";
import MemorySection from "./MemorySection.vue";
import PluginsSection from "./PluginsSection.vue";
import SessionsSection from "./SessionsSection.vue";
import ShortcutSection from "./ShortcutSection.vue";

const SECTION_COMPONENTS: Record<SettingsSectionId, Component> = {
  appearance: AppearanceSection,
  language: LanguageSection,
  shortcut: ShortcutSection,
  chat: ChatSection,
  llm: LlmSection,
  context: ContextSection,
  memory: MemorySection,
  embedding: EmbeddingSection,
  plugins: PluginsSection,
  tools: ToolsSection,
  developer: DeveloperSection,
  sessions: SessionsSection,
};

const { t } = useI18n();
const shell = useShellStore();
const settings = useSettingsStore();

/** 当前分区（带守卫）：分区不存在（如 v2.145 删除的 shell 终端分区，
 * 持久化 id 落在已删分区）或开发者选项上锁时，回落外观（隐藏 = 三处
 * 同隐，面板也不渲染，§3.2 v2.49）。 */
const current = computed(() => {
  const meta = SETTINGS_SECTIONS.find((x) => x.id === shell.settingsSection);
  if (!meta || !isSectionVisible(meta, settings.view?.developer.unlocked ?? false)) {
    return "appearance" as SettingsSectionId;
  }
  return shell.settingsSection;
});
const panel = computed(() => SECTION_COMPONENTS[current.value]);

function sectionLabel(id: SettingsSectionId): string {
  const s = SETTINGS_SECTIONS.find((x) => x.id === id);
  return s ? t(s.labelKey) : id;
}
</script>

<template>
  <div class="flex h-full min-w-0 flex-1">
    <SettingsNav
      :current="current"
      @select="shell.settingsSection = $event"
    />
    <div class="flex min-w-0 flex-1 flex-col bg-page">
      <!-- 56px 目的地页头（§3.31）：当前分区标题 20px/600/-0.02em -->
      <header
        class="flex h-14 shrink-0 items-center gap-3 px-6"
        style="padding-right: var(--lw-wincontrols)"
      >
        <!-- lw-waive: §二⑫ 页面页头标题特例（20px 暂定，报编号翻案） -->
        <h1
          class="truncate text-[20px] font-semibold tracking-[-0.02em] text-ink-1"
        >
          {{ sectionLabel(current) }}
        </h1>
        <div class="h-full min-w-0 flex-1" data-tauri-drag-region />
      </header>
      <div class="scroll-y min-h-0 flex-1">
        <p
          v-if="settings.error"
          class="rounded-[var(--lw-r-xs)] bg-danger/10 px-3 py-2 text-xs text-danger"
        >
          {{ settings.error }}
        </p>
        <!-- v3.6：设置正文列独立 token（--lw-settings-max 1024），w-full 随窗收缩 -->
        <div class="mx-auto w-full max-w-[var(--lw-settings-max)] px-6 py-6">
          <component :is="panel" />
        </div>
      </div>
    </div>
  </div>
</template>
