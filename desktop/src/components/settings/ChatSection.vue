<script setup lang="ts">
/**
 * 设置分区：对话行为（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好）。
 * 列宽偏好走 settings 不走 localStorage（§2.7：随设置同步与备份，拖拽
 * 对话区边缘双柄同写这一份）；保存即生效。
 * 行版式（§3.32·七 v3.5）：左行名右控件；布尔项用 SettingsToggle。
 *
 * S4 刀④ 数值归类：列宽从数字框收成窄/标准/宽三档；拖拽出来的非档值由
 * nearestChatWidthPreset 就近显示，不改存量。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import {
  CHAT_CONTENT_WIDTH_DEFAULT,
  CHAT_CONTENT_WIDTH_NARROW,
  CHAT_CONTENT_WIDTH_WIDE,
  nearestChatWidthPreset,
} from "../../lib/chat-width";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsSegmented from "./SettingsSegmented.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  width: CHAT_CONTENT_WIDTH_DEFAULT,
  collapseToolProcess: false,
  enterToSend: true,
  usageReadout: "used",
  autoTitle: true,
  cacheStats: true,
  foldLongMessages: true,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.width = v.chat.contentMaxWidth ?? CHAT_CONTENT_WIDTH_DEFAULT;
    form.collapseToolProcess = v.chat.collapseToolProcess;
    form.enterToSend = v.chat.enterToSend;
    form.usageReadout = v.chat.usageReadout;
    form.autoTitle = v.chat.autoTitle;
    form.cacheStats = v.chat.cacheStats;
    form.foldLongMessages = v.chat.foldLongMessages;
  },
  { immediate: true },
);

function save() {
  void settings.saveChat({
    contentMaxWidth: Math.round(form.width),
    collapseToolProcess: form.collapseToolProcess,
    enterToSend: form.enterToSend,
    usageReadout: form.usageReadout,
    autoTitle: form.autoTitle,
    cacheStats: form.cacheStats,
    foldLongMessages: form.foldLongMessages,
  });
}

/** 列宽档位点选即存（只发宽度一键，不替用户提交其它未保存的开关）。 */
function saveWidth() {
  void settings.saveChat({ contentMaxWidth: Math.round(form.width) });
}

const widthOptions = computed(() => [
  { value: String(CHAT_CONTENT_WIDTH_NARROW), label: t("settings.chatWidthNarrow") },
  { value: String(CHAT_CONTENT_WIDTH_DEFAULT), label: t("settings.chatWidthStandard") },
  { value: String(CHAT_CONTENT_WIDTH_WIDE), label: t("settings.chatWidthWide") },
]);

/** 分段控件绑 string；点选 = 落到该档并立即保存。 */
const widthPreset = computed({
  get: () => String(nearestChatWidthPreset(form.width)),
  set: (v: string) => {
    form.width = Number(v);
    saveWidth();
  },
});

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
    >
      <SettingsSegmented
        v-model="widthPreset"
        :options="widthOptions"
        :label="t('settings.chatWidthTitle')"
      />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.autoTitleTitle')"
      :hint="t('settings.autoTitleHint')"
    >
      <SettingsToggle v-model="form.autoTitle" :label="t('settings.autoTitleTitle')" />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.collapseToolTitle')"
      :hint="t('settings.collapseToolHint')"
    >
      <SettingsToggle
        v-model="form.collapseToolProcess"
        :label="t('settings.collapseToolTitle')"
      />
    </SettingsRow>

    <SettingsRow :title="t('settings.enterToSendTitle')">
      <SettingsToggle v-model="form.enterToSend" :label="t('settings.enterToSendTitle')" />
    </SettingsRow>

    <!-- 长消息折叠（组件规格「Line Clamp with Fade Mask」）：只有开关，
         行数写死 5（S4 刀④ 数值归类：能确定的数值不进设置面） -->
    <SettingsRow
      :title="t('settings.foldMessagesTitle')"
      :hint="t('settings.foldMessagesHint')"
    >
      <SettingsToggle v-model="form.foldLongMessages" :label="t('settings.foldMessagesTitle')" />
    </SettingsRow>

    <SettingsRow :title="t('settings.usageReadoutTitle')">
      <SettingsSegmented
        v-model="form.usageReadout"
        :options="readoutOptions"
        :label="t('settings.usageReadoutTitle')"
      />
    </SettingsRow>

    <!-- M0 观测（KV v2 §7）：缓存命中率显示（v2.137 单价两行与节省估算
         一并退役——单价要用户手填 = 无用负担） -->
    <SettingsRow
      :title="t('settings.cacheStatsTitle')"
      :hint="t('settings.cacheStatsHint')"
    >
      <SettingsToggle v-model="form.cacheStats" :label="t('settings.cacheStatsTitle')" />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
