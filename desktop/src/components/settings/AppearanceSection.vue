<script setup lang="ts">
/** 设置分区：外观（⑰，settings.json `appearance` 区）。主题三档，
 * 保存即生效（lib/theme.ts 写 <html data-theme>；§六）。 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const theme = computed({
  get: () => settings.view?.appearance.theme ?? "dark",
  set: (v: string) => void settings.setTheme(v),
});

const options = [
  { value: "system", label: "settings.themeSystem" },
  { value: "light", label: "settings.themeLight" },
  { value: "dark", label: "settings.themeDark" },
] as const;
</script>

<template>
  <section class="space-y-2 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.themeTitle") }}</h3>
    <div class="flex w-fit gap-1 rounded-lg border border-line-strong p-1">
      <button
        v-for="o in options"
        :key="o.value"
        class="rounded-md px-3 py-1 text-xs font-medium"
        :class="
          theme === o.value
            ? 'bg-surface-2 text-ink-1'
            : 'text-ink-4 hover:text-ink-2'
        "
        @click="theme = o.value"
      >
        {{ t(o.label) }}
      </button>
    </div>
    <p class="text-[11px] text-ink-4">{{ t("settings.themeHint") }}</p>
  </section>
</template>
