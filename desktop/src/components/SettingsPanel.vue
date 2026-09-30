<script setup lang="ts">
/**
 * 设置面板（壳批次后 = 分区容器，docs/design/UI shell and patterns.md
 * §3.2）：settings.json 分区 ↔ 面板分区一一对应——llm / context（⑨）/
 * shortcut / locale（语言）为既有可写区；sessions 只读展示；memory /
 * developer 为占位（⑪/⑫ 落地）。各分区自管表单与保存动作，本文件
 * 只做纵向堆叠与错误条，不写业务。
 */
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../stores/settings";
import ContextSection from "./settings/ContextSection.vue";
import LanguageSection from "./settings/LanguageSection.vue";
import LlmSection from "./settings/LlmSection.vue";
import MemorySection from "./settings/MemorySection.vue";
import PlaceholderSection from "./settings/PlaceholderSection.vue";
import SessionsSection from "./settings/SessionsSection.vue";
import ShortcutSection from "./settings/ShortcutSection.vue";

const { t } = useI18n();
const settings = useSettingsStore();
</script>

<template>
  <div class="mx-auto w-full max-w-2xl space-y-4 overflow-y-auto p-4">
    <h2 class="text-sm font-semibold">{{ t("settings.title") }}</h2>

    <p
      v-if="settings.error"
      class="rounded-lg border border-red-900/60 bg-red-950/60 px-3 py-2 text-xs text-red-300"
    >
      {{ settings.error }}
    </p>

    <LlmSection />

    <!-- ⑨ 上下文管理：窗口 / 阈值滑条 / 策略三档（占位已由真分区替换） -->
    <ContextSection />

    <!-- ⑪ 记忆分区：注入开关 + 注入条数（占位已由真分区替换） -->
    <MemorySection />

    <SessionsSection />
    <LanguageSection />
    <ShortcutSection />

    <!-- ⑫ 开发者分区（暴露调试面板：Top-N / 分布 / token 节省估算）占位 -->
    <PlaceholderSection
      title-key="settings.developerTitle"
      hint-key="settings.developerHint"
    />
  </div>
</template>
