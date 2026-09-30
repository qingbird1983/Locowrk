<script setup lang="ts">
/** 设置分区：全局快捷键（settings.json `shortcut` 区，保存即重注册）。 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

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
  <section class="space-y-2 rounded-xl border border-neutral-800 p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.shortcutTitle") }}</h3>
    <label class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.shortcut") }}</span>
      <input
        v-model="form.shortcut"
        type="text"
        :placeholder="t('settings.shortcutPlaceholder')"
        class="block w-48 rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5 font-mono placeholder:text-neutral-600"
      />
    </label>
    <div class="flex items-center gap-2">
      <button
        class="rounded-lg bg-neutral-700 px-4 py-1.5 text-xs font-semibold transition hover:bg-neutral-600 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="settings.saving"
        @click="save"
      >
        {{ t("settings.save") }}
      </button>
      <span v-if="settings.savedFlash" class="text-xs text-emerald-400">
        {{ t("settings.saved") }}
      </span>
    </div>
    <p class="text-[11px] text-neutral-500">{{ t("settings.shortcutHint") }}</p>
  </section>
</template>
