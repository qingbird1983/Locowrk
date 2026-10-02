<script setup lang="ts">
/**
 * 设置分区：语言切换（settings.json `locale` 区，立即生效，§13.5）。
 * 行版式（§3.32·七 v3.5）：左行名右控件，就地生效无保存按钮。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const localePref = computed({
  get: () => settings.view?.locale ?? "system",
  set: (v: string) => void settings.setLocale(v),
});
</script>

<template>
  <SettingsGroup>
    <SettingsRow :title="t('settings.langTitle')" :hint="t('settings.langHint')">
      <select
        v-model="localePref"
        class="w-44 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
      >
        <option value="system">{{ t("settings.langSystem") }}</option>
        <option value="zh-CN">{{ t("settings.langZh") }}</option>
        <option value="en">{{ t("settings.langEn") }}</option>
      </select>
    </SettingsRow>
  </SettingsGroup>
</template>
