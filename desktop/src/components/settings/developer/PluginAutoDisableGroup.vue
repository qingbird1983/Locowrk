<script setup lang="ts">
/**
 * 设置分区子块：插件自动停用（S4 刀④ 自「插件」设置分区迁入）。
 * 原分区只剩这一项，迁入后整分区退役——普通用户没有插件数值可配。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../../stores/settings";
import SettingsGroup from "../SettingsGroup.vue";
import SettingsRow from "../SettingsRow.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({ autoDisableIdleDays: 0 });

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.autoDisableIdleDays = v.plugins.autoDisableIdleDays;
  },
  { immediate: true },
);

/** 0 = 关；保存即巡检一遍。 */
function save() {
  void settings.savePlugins({
    autoDisableIdleDays: form.autoDisableIdleDays,
  });
}
</script>

<template>
  <SettingsGroup :title="t('developer.pluginsTitle')" :hint="t('developer.pluginsHint')">
    <SettingsRow
      :title="t('developer.autoDisableDays')"
      :hint="t('developer.autoDisableDaysHint')"
      control-id="dev-auto-disable-days"
    >
      <input
        id="dev-auto-disable-days"
        v-model.number="form.autoDisableIdleDays"
        type="number"
        min="0"
        max="365"
        step="1"
        class="lw-field lw-field-num px-2 text-xs"
        @change="save"
      />
    </SettingsRow>
  </SettingsGroup>
</template>
