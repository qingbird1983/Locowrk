<script setup lang="ts">
/**
 * 设置分区：全局快捷键（settings.json `shortcut` 区，保存即重注册）。
 * 行版式（§3.32·七 v3.5）：左行名右控件。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({ shortcut: "" });
watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.shortcut = v.shortcut;
  },
  { immediate: true },
);

function save() {
  void settings.saveShortcut(form.shortcut.trim());
}
</script>

<template>
  <SettingsGroup>
    <SettingsRow
      :title="t('settings.shortcut')"
      :hint="t('settings.shortcutHint')"
      control-id="shortcut-input"
    >
      <input
        id="shortcut-input"
        v-model="form.shortcut"
        type="text"
        :placeholder="t('settings.shortcutPlaceholder')"
        class="w-44 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 font-mono text-xs placeholder:text-ink-5"
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
