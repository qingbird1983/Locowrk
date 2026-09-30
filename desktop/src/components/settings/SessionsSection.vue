<script setup lang="ts">
/** 设置分区：会话存储位置（只读展示，settings.json 无对应可写区）。 */
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();
</script>

<template>
  <section class="space-y-2 rounded-xl border border-neutral-800 p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.sessionsTitle") }}</h3>
    <p class="text-xs text-neutral-400">{{ t("settings.dataDir") }}</p>
    <code class="block overflow-x-auto rounded bg-neutral-900/80 px-2 py-1.5 font-mono text-[11px] text-neutral-300">
      {{ settings.view?.dataDir ?? "…" }}
    </code>
    <p class="text-[11px] text-neutral-500">{{ t("settings.dataDirHint") }}</p>
    <p class="text-[11px] text-neutral-500">{{ t("settings.sessionsMemory") }}</p>
  </section>
</template>
