<script setup lang="ts">
/**
 * 设置分区：embedding 模型（⑬ 智能层共享基建，settings.json `embedding`
 * 区）。工具语义检索 / 经历记忆 / 隐式提取都靠它把文本变向量；与主模型
 * 解耦——主模型没有 embedding 端点（如 DeepSeek）也能配。三项全空 =
 * 智能层整体缺席（自动降级：检索退关键词、经历不写不注入），不报错。
 * "检测"按钮嵌一条测试文本验证端点连通并显示向量维度；API Key 只落盘
 * 不出 IPC（同主 Key 纪律）。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { llmProbeEmbedding } from "../../lib/api";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  model: "",
  baseUrl: "",
  apiKey: "",
  probing: false,
  probeError: "",
  probeDim: 0,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.model = v.embedding.model ?? "";
    form.baseUrl = v.embedding.baseUrl ?? "";
  },
  { immediate: true },
);

/** 兜底提示：未填端点时展示解析来源（ollama 本机 / 主模型端点）。 */
const fallbackHint = (
  baseUrl: string | null | undefined,
  provider: string | undefined,
): string => {
  if (baseUrl) return baseUrl;
  if (provider === "ollama") return "http://localhost:11434";
  return "";
};

async function probe() {
  if (form.probing || !form.model.trim()) return;
  form.probing = true;
  form.probeError = "";
  form.probeDim = 0;
  try {
    form.probeDim = await llmProbeEmbedding(
      form.baseUrl || null,
      form.model.trim(),
      form.apiKey || undefined,
    );
  } catch (e) {
    form.probeError = String(e);
  } finally {
    form.probing = false;
  }
}

function save() {
  void settings.saveEmbedding({
    model: form.model,
    baseUrl: form.baseUrl, // 空串 = 清除，回落 provider 兜底
    apiKey: form.apiKey || undefined, // 留空 = 不改动已存 Key
  });
  form.apiKey = "";
}

function clearKey() {
  void settings.saveEmbedding({ apiKey: "" });
}
</script>

<template>
  <section class="space-y-3 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.embeddingTitle") }}</h3>
    <p class="text-[11px] text-ink-4">{{ t("settings.embeddingHint") }}</p>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.embeddingModel") }}</span>
      <input
        v-model="form.model"
        type="text"
        placeholder="nomic-embed-text / text-embedding-3-small"
        class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 placeholder:text-ink-5"
      />
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.embeddingBaseUrl") }}</span>
      <input
        v-model="form.baseUrl"
        type="text"
        :placeholder="fallbackHint(settings.view?.embedding.baseUrl, settings.view?.llm.provider ?? '') || t('settings.embeddingBaseUrlPlaceholder')"
        class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 placeholder:text-ink-5"
      />
      <span class="text-[11px] text-ink-5">{{ t("settings.embeddingBaseUrlHint") }}</span>
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.embeddingApiKey") }}</span>
      <input
        v-model="form.apiKey"
        type="password"
        autocomplete="off"
        :placeholder="settings.view?.embedding.apiKeySet ? t('settings.apiKeyKeep') : t('settings.embeddingApiKeyPlaceholder')"
        class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 placeholder:text-ink-5"
      />
    </label>

    <div class="flex items-center gap-2">
      <button
        class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-2 transition hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="form.probing"
        @click="probe"
      >
        {{ form.probing ? t("settings.embeddingProbing") : t("settings.embeddingDetect") }}
      </button>
      <span v-if="form.probeDim > 0" class="text-xs text-emerald-400">
        {{ t("settings.embeddingDim", { n: form.probeDim }) }}
      </span>
    </div>
    <p v-if="form.probeError" class="text-[11px] text-red-300">
      {{ t("settings.embeddingDetectFail") }} {{ form.probeError }}
    </p>

    <div class="flex items-center gap-2">
      <button
        class="rounded-lg bg-surface-2 px-4 py-1.5 text-xs font-semibold transition hover:bg-surface-3 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="settings.saving"
        @click="save"
      >
        {{ t("settings.save") }}
      </button>
      <button
        v-if="settings.view?.embedding.apiKeySet"
        class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-3 transition hover:bg-surface-1"
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
