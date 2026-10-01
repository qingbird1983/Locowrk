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
  <section class="space-y-3 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.pluginsTitle") }}</h3>
    <p class="text-[11px] text-ink-4">{{ t("settings.pluginsHint") }}</p>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.autoDisableDays") }}</span>
      <input
        v-model.number="form.autoDisableIdleDays"
        type="number"
        min="0"
        max="365"
        step="1"
        class="block w-32 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5"
      />
      <span class="text-[11px] text-ink-5">{{ t("settings.autoDisableDaysHint") }}</span>
    </label>

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
  </section>
</template>
