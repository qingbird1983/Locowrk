<script setup lang="ts">
/**
 * 设置分区子块：工具执行参数（S4 刀④ 自「工具与分发」迁入）。
 * 普通用户不碰这些上限；每行 change 即保存该键，下一轮请求生效。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../../stores/settings";
import type { ToolsUpdate } from "../../../types";
import SettingsGroup from "../SettingsGroup.vue";
import SettingsRow from "../SettingsRow.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  maxPerRequest: 20,
  callTimeoutSecs: 30,
  maxTurnSteps: 8,
  approvalTimeoutSecs: 100,
  askTimeoutSecs: 300,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.maxPerRequest = v.tools.maxPerRequest;
    form.callTimeoutSecs = v.tools.callTimeoutSecs;
    form.maxTurnSteps = v.tools.maxTurnSteps;
    form.approvalTimeoutSecs = v.tools.approvalTimeoutSecs;
    form.askTimeoutSecs = v.tools.askTimeoutSecs;
  },
  { immediate: true },
);

function save(patch: ToolsUpdate) {
  void settings.saveTools(patch);
}
</script>

<template>
  <SettingsGroup :title="t('developer.toolsTitle')" :hint="t('developer.toolsHint')">
    <SettingsRow
      :title="t('developer.toolsMax')"
      :hint="t('developer.toolsMaxHint')"
      control-id="dev-tools-max"
    >
      <input
        id="dev-tools-max"
        v-model.number="form.maxPerRequest"
        type="number"
        min="1"
        max="200"
        class="lw-field lw-field-num px-2 text-xs"
        @change="save({ maxPerRequest: form.maxPerRequest })"
      />
    </SettingsRow>

    <SettingsRow
      :title="t('developer.callTimeout')"
      :hint="t('developer.callTimeoutHint')"
      control-id="dev-tools-call-timeout"
    >
      <input
        id="dev-tools-call-timeout"
        v-model.number="form.callTimeoutSecs"
        type="number"
        min="5"
        max="3600"
        class="lw-field lw-field-num px-2 text-xs"
        @change="save({ callTimeoutSecs: form.callTimeoutSecs })"
      />
    </SettingsRow>

    <SettingsRow
      :title="t('developer.maxTurnSteps')"
      :hint="t('developer.maxTurnStepsHint')"
      control-id="dev-tools-max-turn-steps"
    >
      <input
        id="dev-tools-max-turn-steps"
        v-model.number="form.maxTurnSteps"
        type="number"
        min="1"
        max="100"
        class="lw-field lw-field-num px-2 text-xs"
        @change="save({ maxTurnSteps: form.maxTurnSteps })"
      />
    </SettingsRow>

    <SettingsRow
      :title="t('developer.approvalTimeout')"
      :hint="t('developer.approvalTimeoutHint')"
      control-id="dev-tools-approval-timeout"
    >
      <input
        id="dev-tools-approval-timeout"
        v-model.number="form.approvalTimeoutSecs"
        type="number"
        min="0"
        max="86400"
        class="lw-field lw-field-num px-2 text-xs"
        @change="save({ approvalTimeoutSecs: form.approvalTimeoutSecs })"
      />
    </SettingsRow>

    <SettingsRow
      :title="t('developer.askTimeout')"
      :hint="t('developer.askTimeoutHint')"
      control-id="dev-tools-ask-timeout"
    >
      <input
        id="dev-tools-ask-timeout"
        v-model.number="form.askTimeoutSecs"
        type="number"
        min="0"
        max="86400"
        class="lw-field lw-field-num px-2 text-xs"
        @change="save({ askTimeoutSecs: form.askTimeoutSecs })"
      />
    </SettingsRow>
  </SettingsGroup>
</template>
