<script setup lang="ts">
/**
 * 设置分区：外观（⑰，settings.json `appearance` 区）。主题三档，
 * 保存即生效（lib/theme.ts 写 <html data-theme>；§六）。
 * 行版式（§3.32·七 v3.5）：左行名右控件，就地生效无保存按钮。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";

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
  <SettingsGroup>
    <SettingsRow :title="t('settings.themeTitle')" :hint="t('settings.themeHint')">
      <div class="flex w-fit gap-1 rounded-lg border border-line-strong p-1">
        <button
          v-for="o in options"
          :key="o.value"
          type="button"
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
    </SettingsRow>
  </SettingsGroup>
</template>
