<script setup lang="ts">
/**
 * 设置分区堆叠（UI shell doc §3.32）：按 lib/settings-index.ts 的索引
 * 顺序渲染全部分区——"settings.json 分区 ↔ 面板分区一一对应"由索引
 * 驱动机械保证（索引里有而这里没组件 = 构建期类型错误）。分区包装元素
 * 带 data-settings-section 锚，供 SettingsPage 滚动侦测 / ⌘K 跳转 /
 * 锚点闪烁定位；分区之间 gap 32px（§3.32 七）。各分区自管表单与保存，
 * 本文件不写业务。
 */
import type { Component } from "vue";
import { useSettingsStore } from "../stores/settings";
import {
  SETTINGS_SECTIONS,
  type SettingsSectionId,
} from "../lib/settings-index";
import AppearanceSection from "./settings/AppearanceSection.vue";
import ContextSection from "./settings/ContextSection.vue";
import DeveloperSection from "./settings/DeveloperSection.vue";
import EmbeddingSection from "./settings/EmbeddingSection.vue";
import LanguageSection from "./settings/LanguageSection.vue";
import LlmSection from "./settings/LlmSection.vue";
import MemorySection from "./settings/MemorySection.vue";
import PluginsSection from "./settings/PluginsSection.vue";
import SessionsSection from "./settings/SessionsSection.vue";
import ShellSection from "./settings/ShellSection.vue";
import ShortcutSection from "./settings/ShortcutSection.vue";

const settings = useSettingsStore();

const SECTION_COMPONENTS: Record<SettingsSectionId, Component> = {
  appearance: AppearanceSection,
  language: LanguageSection,
  shortcut: ShortcutSection,
  llm: LlmSection,
  context: ContextSection,
  memory: MemorySection,
  embedding: EmbeddingSection,
  plugins: PluginsSection,
  shell: ShellSection,
  tools: DeveloperSection,
  sessions: SessionsSection,
};
</script>

<template>
  <div
    class="mx-auto w-full max-w-[var(--lw-content-max)] space-y-8 px-6 py-6"
  >
    <p
      v-if="settings.error"
      class="rounded-lg bg-danger/10 px-3 py-2 text-xs text-danger"
    >
      {{ settings.error }}
    </p>

    <section
      v-for="s in SETTINGS_SECTIONS"
      :id="`settings-section-${s.id}`"
      :key="s.id"
      :data-settings-section="s.id"
    >
      <component :is="SECTION_COMPONENTS[s.id]" />
    </section>
  </div>
</template>
