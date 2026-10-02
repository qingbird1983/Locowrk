<script setup lang="ts">
/**
 * 设置分区：外观（⑰，settings.json `appearance` 区）。主题三档，
 * 保存即生效（lib/theme.ts 写 <html data-theme>；§六）。
 * 行版式（§3.32·七 v3.5）：左行名右控件，就地生效无保存按钮。
 * 分段控件走 SettingsSegmented（v3.6 两色控件体系：白底台面 + 选中灰）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSegmented from "./SettingsSegmented.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const theme = computed({
  get: () => settings.view?.appearance.theme ?? "dark",
  set: (v: string) => void settings.setTheme(v),
});

const options = computed(() => [
  { value: "system", label: t("settings.themeSystem") },
  { value: "light", label: t("settings.themeLight") },
  { value: "dark", label: t("settings.themeDark") },
]);
</script>

<template>
  <SettingsGroup>
    <SettingsRow :title="t('settings.themeTitle')" :hint="t('settings.themeHint')">
      <SettingsSegmented
        v-model="theme"
        :options="options"
        :label="t('settings.themeTitle')"
      />
    </SettingsRow>
  </SettingsGroup>
</template>
