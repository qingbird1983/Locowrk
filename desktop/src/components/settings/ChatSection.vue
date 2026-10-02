<script setup lang="ts">
/**
 * 设置分区：对话行为（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好）。
 * 列宽偏好走 settings 不走 localStorage（§2.7：随设置同步与备份，拖拽
 * 对话区边缘双柄同写这一份）；保存即生效。
 * 行版式（§3.32·七 v3.5）：左行名右控件；布尔项用 SettingsToggle。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { CHAT_CONTENT_WIDTH_DEFAULT, CHAT_CONTENT_WIDTH_MIN } from "../../lib/chat-width";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsSegmented from "./SettingsSegmented.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  width: CHAT_CONTENT_WIDTH_DEFAULT,
  expandToolProcess: false,
  enterToSend: true,
  usageReadout: "used",
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.width = v.chat.contentMaxWidth ?? CHAT_CONTENT_WIDTH_DEFAULT;
    form.expandToolProcess = v.chat.expandToolProcess;
    form.enterToSend = v.chat.enterToSend;
    form.usageReadout = v.chat.usageReadout;
  },
  { immediate: true },
);

function save() {
  void settings.saveChat({
    contentMaxWidth: Math.round(form.width),
    expandToolProcess: form.expandToolProcess,
    enterToSend: form.enterToSend,
    usageReadout: form.usageReadout,
  });
}

const readoutOptions = computed(() => [
  { value: "used", label: t("settings.usageReadout_used") },
  { value: "remaining", label: t("settings.usageReadout_remaining") },
]);
</script>

<template>
  <SettingsGroup>
    <SettingsRow
      :title="t('settings.chatWidthTitle')"
      :hint="t('settings.chatWidthHint')"
      control-id="chat-width-input"
    >
      <input
        id="chat-width-input"
        v-model.number="form.width"
        type="number"
        :min="CHAT_CONTENT_WIDTH_MIN"
        :max="4000"
        step="20"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <SettingsRow :title="t('settings.expandToolTitle')">
      <SettingsToggle v-model="form.expandToolProcess" :label="t('settings.expandToolTitle')" />
    </SettingsRow>

    <SettingsRow :title="t('settings.enterToSendTitle')">
      <SettingsToggle v-model="form.enterToSend" :label="t('settings.enterToSendTitle')" />
    </SettingsRow>

    <SettingsRow :title="t('settings.usageReadoutTitle')">
      <SettingsSegmented
        v-model="form.usageReadout"
        :options="readoutOptions"
        :label="t('settings.usageReadoutTitle')"
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
