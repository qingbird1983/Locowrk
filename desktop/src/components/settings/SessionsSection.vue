<script setup lang="ts">
/**
 * 设置分区：会话存储位置（只读展示，settings.json 无对应可写区）。
 * 路径这类天然满宽的只读信息按 §3.32·七 v3.5 整块一卡呈现，不硬塞行式。
 */
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();
</script>

<template>
  <div class="space-y-3">
    <div class="rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-3.5">
      <p class="text-[13px] font-medium leading-tight text-ink-1">
        {{ t("settings.dataDir") }}
      </p>
      <p class="mt-1 text-[11.5px] leading-relaxed text-ink-4">
        {{ t("settings.dataDirHint") }}
      </p>
      <code
        class="mt-2 block overflow-x-auto rounded-[var(--lw-r-3xs)] bg-inset px-2 py-1.5 font-mono text-[11px] text-ink-2"
      >
        {{ settings.view?.dataDir ?? "…" }}
      </code>
    </div>
    <p class="px-1 text-[11.5px] leading-relaxed text-ink-4">
      {{ t("settings.sessionsMemory") }}
    </p>
  </div>
</template>
