<script setup lang="ts">
/** 设置分区：语言切换（settings.json `locale` 区，立即生效，§13.5）。 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const localePref = computed({
  get: () => settings.view?.locale ?? "system",
  set: (v: string) => void settings.setLocale(v),
});
</script>

<template>
  <section class="space-y-2 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.langTitle") }}</h3>
    <select v-model="localePref" class="block w-48 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs">
      <option value="system">{{ t("settings.langSystem") }}</option>
      <option value="zh-CN">{{ t("settings.langZh") }}</option>
      <option value="en">{{ t("settings.langEn") }}</option>
    </select>
    <p class="text-[11px] text-ink-4">{{ t("settings.langHint") }}</p>
  </section>
</template>
