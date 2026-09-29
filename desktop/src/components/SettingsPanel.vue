<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

// LLM 表单是投影的本地可编辑副本；apiKey 输入框永远从空开始
//（原值不出后端），placeholder 按 apiKeySet 提示“留空保持不变”。
const form = reactive({
  provider: "openai",
  model: "",
  baseUrl: "",
  apiKey: "",
  shortcut: "",
});
watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.provider = v.llm.provider;
    form.model = v.llm.model;
    form.baseUrl = v.llm.baseUrl ?? "";
    form.shortcut = v.shortcut;
  },
  { immediate: true },
);

const localePref = computed({
  get: () => settings.view?.locale ?? "system",
  set: (v: string) => void settings.setLocale(v),
});

function saveLlm() {
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

function saveShortcut() {
  void settings.saveShortcut(form.shortcut.trim());
}
</script>

<template>
  <div class="mx-auto w-full max-w-2xl space-y-4 overflow-y-auto p-4">
    <h2 class="text-sm font-semibold">{{ t("settings.title") }}</h2>

    <p
      v-if="settings.error"
      class="rounded-lg border border-red-900/60 bg-red-950/60 px-3 py-2 text-xs text-red-300"
    >
      {{ settings.error }}
    </p>

    <!-- 块一：LLM 配置（阶段八·接真实模型时生效） -->
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
          @click="saveLlm"
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

    <!-- 块二：会话存储位置（只读展示） -->
    <section class="space-y-2 rounded-xl border border-neutral-800 p-4">
      <h3 class="text-xs font-semibold">{{ t("settings.sessionsTitle") }}</h3>
      <p class="text-xs text-neutral-400">{{ t("settings.dataDir") }}</p>
      <code class="block overflow-x-auto rounded bg-neutral-900/80 px-2 py-1.5 font-mono text-[11px] text-neutral-300">
        {{ settings.view?.dataDir ?? "…" }}
      </code>
      <p class="text-[11px] text-neutral-500">{{ t("settings.dataDirHint") }}</p>
      <p class="text-[11px] text-neutral-500">{{ t("settings.sessionsMemory") }}</p>
    </section>

    <!-- 块三：语言切换（立即生效 + 持久化，§13.5） -->
    <section class="space-y-2 rounded-xl border border-neutral-800 p-4">
      <h3 class="text-xs font-semibold">{{ t("settings.langTitle") }}</h3>
      <select v-model="localePref" class="block w-48 rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5 text-xs">
        <option value="system">{{ t("settings.langSystem") }}</option>
        <option value="zh-CN">{{ t("settings.langZh") }}</option>
        <option value="en">{{ t("settings.langEn") }}</option>
      </select>
      <p class="text-[11px] text-neutral-500">{{ t("settings.langHint") }}</p>
    </section>

    <!-- 块四：全局快捷键（阶段八⑤：托盘 + 快捷键，保存即重注册） -->
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
          @click="saveShortcut"
        >
          {{ t("settings.save") }}
        </button>
        <span v-if="settings.savedFlash" class="text-xs text-emerald-400">
          {{ t("settings.saved") }}
        </span>
      </div>
      <p class="text-[11px] text-neutral-500">{{ t("settings.shortcutHint") }}</p>
    </section>
  </div>
</template>
