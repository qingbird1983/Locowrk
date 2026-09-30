<script setup lang="ts">
/**
 * 设置分区：LLM 配置（settings.json `llm` 区，§13.1）。保存即生效，
 * API Key 只落盘不出 IPC——输入框永远从空开始，placeholder 按
 * apiKeySet 提示"留空保持不变"。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

// 表单是投影的本地可编辑副本
const form = reactive({
  provider: "openai",
  model: "",
  baseUrl: "",
  apiKey: "",
});
watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.provider = v.llm.provider;
    form.model = v.llm.model;
    form.baseUrl = v.llm.baseUrl ?? "";
  },
  { immediate: true },
);

function save() {
  void settings.saveLlm({
    provider: form.provider,
    model: form.model,
    baseUrl: form.baseUrl, // 空串 = 清除，回落 provider 默认端点
    apiKey: form.apiKey || undefined, // 留空 = 不改动已存 Key
  });
  form.apiKey = "";
}

function clearKey() {
  void settings.saveLlm({ apiKey: "" });
}
</script>

<template>
  <section class="space-y-3 rounded-xl border border-neutral-800 p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.llmTitle") }}</h3>
    <p class="text-[11px] text-neutral-500">{{ t("settings.llmHint") }}</p>

    <label class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.provider") }}</span>
      <select v-model="form.provider" class="block w-full rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5">
        <option value="openai">OpenAI</option>
        <option value="anthropic">Anthropic</option>
        <option value="ollama">Ollama</option>
      </select>
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.model") }}</span>
      <input
        v-model="form.model"
        type="text"
        class="block w-full rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5"
      />
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.baseUrl") }}</span>
      <input
        v-model="form.baseUrl"
        type="text"
        :placeholder="t('settings.baseUrlPlaceholder')"
        class="block w-full rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5 placeholder:text-neutral-600"
      />
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.apiKey") }}</span>
      <input
        v-model="form.apiKey"
        type="password"
        autocomplete="off"
        :placeholder="settings.view?.llm.apiKeySet ? t('settings.apiKeyKeep') : t('settings.apiKeyPlaceholder')"
        class="block w-full rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5 placeholder:text-neutral-600"
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
      <button
        v-if="settings.view?.llm.apiKeySet"
        class="rounded-lg border border-neutral-700 px-3 py-1.5 text-xs text-neutral-400 transition hover:bg-neutral-800"
        :disabled="settings.saving"
        @click="clearKey"
      >
        {{ t("settings.clearKey") }}
      </button>
      <span v-if="settings.savedFlash" class="text-xs text-emerald-400">
        {{ t("settings.saved") }}
      </span>
    </div>
  </section>
</template>
