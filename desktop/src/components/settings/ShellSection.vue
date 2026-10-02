<script setup lang="ts">
/**
 * 设置分区：命令终端治理（settings.json `shell` 区，S1；清单 §2.7/§3
 * 约束最小化）。高危命令解锁开关——默认 rm/sudo/chmod 等直接拒绝，
 * 开启后改为进审批（仍要点确认，不是直接放行）；解释器/包装/网络类
 * 保持硬锁。开关经插件加载前的 env 注入，重启后生效。
 * 行版式（§3.32·七 v3.5）：开关行 + 警告/重启提示按 §四 写在行外下方。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
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
      <SettingsToggle v-model="form.unlockRisky" :label="t('settings.shellUnlock')" />
    </SettingsRow>
  </SettingsGroup>

  <div class="space-y-2 px-1">
    <p class="rounded-lg bg-warn/10 p-2 text-[11px] leading-relaxed text-warn">
      ⚠️ {{ t("settings.shellUnlockWarning") }}
    </p>
    <p class="text-[11px] text-ink-5">{{ t("settings.shellRestart") }}</p>
    <SettingsSaveRow class="pt-1" @save="save" />
  </div>
</template>
