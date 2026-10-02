<script setup lang="ts">
/**
 * 设置分区：语言切换（settings.json `locale` 区，立即生效，§13.5）。
 * 行版式（§3.32·七 v3.5）：左行名右控件，就地生效无保存按钮。
 * 下拉走 SettingsSelect（v3.6 两色控件体系：白底台面触发器 + 白面板）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSelect from "./SettingsSelect.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const localePref = computed({
  get: () => settings.view?.locale ?? "system",
  set: (v: string) => void settings.setLocale(v),
});

const options = computed(() => [
  { value: "system", label: t("settings.langSystem") },
  { value: "zh-CN", label: t("settings.langZh") },
  { value: "en", label: t("settings.langEn") },
]);
</script>

<template>
  <SettingsGroup>
    <SettingsRow :title="t('settings.langTitle')" :hint="t('settings.langHint')">
      <SettingsSelect
        v-model="localePref"
        :entries="options"
        :label="t('settings.langTitle')"
      />
    </SettingsRow>
  </SettingsGroup>
</template>
