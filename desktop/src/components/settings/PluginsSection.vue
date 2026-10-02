<script setup lang="ts">
/**
 * 设置分区：插件管理（⑬，settings.json `plugins` 区）。目前只有自动停
 * 用一项——"N 天未使用的插件自动停用"（exposure 文档 §8.8 唯一例外条
 * 款，缺省 0 = 关）。只作用于安装态第三方插件（内置插件不参与闲置评估，
 * 与插件页闲置徽章同口径）；关闭后不会自动恢复已停用的插件。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  autoDisableIdleDays: 0,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.autoDisableIdleDays = v.plugins.autoDisableIdleDays;
  },
  { immediate: true },
);

function save() {
  void settings.savePlugins({
    autoDisableIdleDays: form.autoDisableIdleDays,
  });
}
</script>

<template>
  <SettingsGroup :hint="t('settings.pluginsHint')">
    <SettingsRow
      :title="t('settings.autoDisableDays')"
      :hint="t('settings.autoDisableDaysHint')"
      control-id="plugins-auto-disable-days"
    >
      <input
        id="plugins-auto-disable-days"
        v-model.number="form.autoDisableIdleDays"
        type="number"
        min="0"
        max="365"
        step="1"
        class="w-[140px] rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
