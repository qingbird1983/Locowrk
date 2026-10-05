<script setup lang="ts">
/**
 * 设置分区：命令终端治理（settings.json `shell` 区，S1；清单 §2.7/§3
 * 约束最小化）。高危命令解锁开关——默认 rm/sudo/chmod 等直接拒绝，
 * 开启后改为进审批（仍要点确认，不是直接放行）；解释器/包装/网络类
 * 保持硬锁。开关经插件加载前的 env 注入，重启后生效。
 * 行版式（§3.32·七 v3.5）：开关行；v3.7 起行下常驻警告条退役——行名尾挂
 * HoverTip 警示气泡（warn ⚠ + hover 白底气泡承载解锁后果与重启提示，
 * 移出即隐）。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import HoverTip from "./HoverTip.vue";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  unlockRisky: false,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.unlockRisky = v.shell.unlockRisky;
  },
  { immediate: true },
);

function save() {
  void settings.saveShell({ unlockRisky: form.unlockRisky });
}
</script>

<template>
  <SettingsGroup :hint="t('settings.shellHint')">
    <SettingsRow :title="t('settings.shellUnlock')">
      <template #title-extra>
        <HoverTip icon :label="t('settings.shellUnlock')">
          <p class="text-warn">{{ t("settings.shellUnlockWarning") }}</p>
          <p class="mt-1 text-ink-3">{{ t("settings.shellRestart") }}</p>
        </HoverTip>
      </template>
      <SettingsToggle v-model="form.unlockRisky" :label="t('settings.shellUnlock')" />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
