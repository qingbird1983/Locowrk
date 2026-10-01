<script setup lang="ts">
/**
 * 设置分区：对话行为（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好）。
 * 列宽偏好走 settings 不走 localStorage（§2.7：随设置同步与备份，拖拽
 * 对话区边缘双柄同写这一份）；保存即生效。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { CHAT_CONTENT_WIDTH_DEFAULT, CHAT_CONTENT_WIDTH_MIN } from "../../lib/chat-width";

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
</script>

<template>
  <section class="space-y-3 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.chatTitle") }}</h3>

    <div class="space-y-1">
      <label class="block text-xs" for="chat-width-input">
        {{ t("settings.chatWidthTitle") }}
      </label>
      <input
        id="chat-width-input"
        v-model.number="form.width"
        type="number"
        :min="CHAT_CONTENT_WIDTH_MIN"
        :max="4000"
        step="20"
        class="w-28 rounded-lg border border-line bg-page px-2 py-1 text-xs outline-none"
      />
      <p class="text-[11px] text-ink-4">{{ t("settings.chatWidthHint") }}</p>
    </div>

    <label class="flex cursor-pointer items-start gap-2 text-xs">
      <input
        v-model="form.expandToolProcess"
        type="checkbox"
        class="mt-0.5 accent-[var(--lw-accent)]"
      />
      <span>{{ t("settings.expandToolTitle") }}</span>
    </label>

    <label class="flex cursor-pointer items-start gap-2 text-xs">
      <input
        v-model="form.enterToSend"
        type="checkbox"
        class="mt-0.5 accent-[var(--lw-accent)]"
      />
      <span>{{ t("settings.enterToSendTitle") }}</span>
    </label>

    <div class="space-y-1">
      <span class="block text-xs">{{ t("settings.usageReadoutTitle") }}</span>
      <div class="flex w-fit gap-1 rounded-lg border border-line-strong p-1">
        <button
          v-for="o in ['used', 'remaining']"
          :key="o"
          type="button"
          class="rounded-md px-3 py-1 text-xs font-medium"
          :class="
            form.usageReadout === o
              ? 'bg-surface-2 text-ink-1'
              : 'text-ink-4 hover:text-ink-2'
          "
          @click="form.usageReadout = o"
        >
          {{ t(`settings.usageReadout_${o}`) }}
        </button>
      </div>
    </div>

    <div class="flex items-center gap-2">
      <button
        class="rounded-lg bg-wash-2 px-4 py-1.5 text-xs font-semibold transition hover:bg-wash-3 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="settings.saving"
        @click="save"
      >
        {{ t("settings.save") }}
      </button>
      <span v-if="settings.savedFlash" class="text-xs text-ok">
        {{ t("settings.saved") }}
      </span>
    </div>
    <p v-if="settings.error" class="rounded-lg bg-danger/10 p-2 text-[11px] text-danger">
      {{ settings.error }}
    </p>
  </section>
</template>
