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
  autoTitle: true,
  cacheStats: true,
  cacheHitPrice: 0.02,
  cacheMissPrice: 1.0,
  foldLongMessages: true,
  foldLines: 5,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.width = v.chat.contentMaxWidth ?? CHAT_CONTENT_WIDTH_DEFAULT;
    form.expandToolProcess = v.chat.expandToolProcess;
    form.enterToSend = v.chat.enterToSend;
    form.usageReadout = v.chat.usageReadout;
    form.autoTitle = v.chat.autoTitle;
    form.cacheStats = v.chat.cacheStats;
    form.cacheHitPrice = v.chat.cacheHitPrice;
    form.cacheMissPrice = v.chat.cacheMissPrice;
    form.foldLongMessages = v.chat.foldLongMessages;
    form.foldLines = v.chat.foldLines;
  },
  { immediate: true },
);

function save() {
  void settings.saveChat({
    contentMaxWidth: Math.round(form.width),
    expandToolProcess: form.expandToolProcess,
    enterToSend: form.enterToSend,
    usageReadout: form.usageReadout,
    autoTitle: form.autoTitle,
    cacheStats: form.cacheStats,
    cacheHitPrice: form.cacheHitPrice,
    cacheMissPrice: form.cacheMissPrice,
    foldLongMessages: form.foldLongMessages,
    foldLines: form.foldLines,
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

    <SettingsRow
      :title="t('settings.autoTitleTitle')"
      :hint="t('settings.autoTitleHint')"
    >
      <SettingsToggle v-model="form.autoTitle" :label="t('settings.autoTitleTitle')" />
    </SettingsRow>

    <SettingsRow :title="t('settings.expandToolTitle')">
      <SettingsToggle v-model="form.expandToolProcess" :label="t('settings.expandToolTitle')" />
    </SettingsRow>

    <SettingsRow :title="t('settings.enterToSendTitle')">
      <SettingsToggle v-model="form.enterToSend" :label="t('settings.enterToSendTitle')" />
    </SettingsRow>

    <!-- 长消息折叠（组件规格「Line Clamp with Fade Mask」）：开关 + 行数 -->
    <SettingsRow
      :title="t('settings.foldMessagesTitle')"
      :hint="t('settings.foldMessagesHint')"
    >
      <SettingsToggle v-model="form.foldLongMessages" :label="t('settings.foldMessagesTitle')" />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.foldLinesTitle')"
      :hint="t('settings.foldLinesHint')"
      control-id="chat-fold-lines"
    >
      <input
        id="chat-fold-lines"
        v-model.number="form.foldLines"
        type="number"
        :min="3"
        :max="20"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <SettingsRow :title="t('settings.usageReadoutTitle')">
      <SettingsSegmented
        v-model="form.usageReadout"
        :options="readoutOptions"
        :label="t('settings.usageReadoutTitle')"
      />
    </SettingsRow>

    <!-- M0 观测（KV v2 §7）：缓存命中率/节省显示与单价（元/百万 tokens） -->
    <SettingsRow
      :title="t('settings.cacheStatsTitle')"
      :hint="t('settings.cacheStatsHint')"
    >
      <SettingsToggle v-model="form.cacheStats" :label="t('settings.cacheStatsTitle')" />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.cacheHitPriceTitle')"
      :hint="t('settings.cachePriceHint')"
      control-id="chat-cache-hit-price"
    >
      <input
        id="chat-cache-hit-price"
        v-model.number="form.cacheHitPrice"
        type="number"
        :min="0"
        :max="10000"
        step="0.01"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.cacheMissPriceTitle')"
      :hint="t('settings.cachePriceHint')"
      control-id="chat-cache-miss-price"
    >
      <input
        id="chat-cache-miss-price"
        v-model.number="form.cacheMissPrice"
        type="number"
        :min="0"
        :max="10000"
        step="0.1"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
