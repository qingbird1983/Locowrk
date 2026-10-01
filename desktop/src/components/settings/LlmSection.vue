<script setup lang="ts">
/**
 * 设置分区：LLM 配置（settings.json `llm` 区，§13.1）。保存即生效，
 * API Key 只落盘不出 IPC——输入框永远从空开始，placeholder 按
 * apiKeySet 提示"留空保持不变"。
 * S2（清单 §5 信息归属）：模型上下文窗口是模型属性，下拉自上下文管理
 * 区迁入；Ollama 走 /api/show 自动检测，云端模型查静态档案兜底。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { compactNumber } from "../../lib/format";
import { profileWindow } from "../../lib/model-profiles";
import { llmProbeWindow } from "../../lib/api";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const PRESETS = [8192, 16384, 32768, 65536, 131072, 200000, 262144, 1048576];

// 表单是投影的本地可编辑副本；窗口用"选项 + 自定义"两段表达
const form = reactive({
  provider: "openai",
  model: "",
  baseUrl: "",
  apiKey: "",
  windowChoice: "unset" as string, // "unset" | preset 数字串 | "custom"
  customWindow: 32768,
  probing: false,
  probeError: "",
});
watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.provider = v.llm.provider;
    form.model = v.llm.model;
    form.baseUrl = v.llm.baseUrl ?? "";
    const w = v.llm.windowTokens;
    form.windowChoice = w == null ? "unset" : String(w);
  },
  { immediate: true },
);

// 静态档案自动填充：只在"未设置"时命中，不覆盖手动选择/检测值
watch(
  () => form.model,
  (m) => {
    if (form.windowChoice !== "unset") return;
    const hit = profileWindow(m);
    if (hit != null) form.windowChoice = String(hit);
  },
);

/** 表单当前生效的窗口（null = 未设置，回落保守保护）。 */
const formWindow = computed<number | null>(() => {
  if (form.windowChoice === "unset") return null;
  if (form.windowChoice === "custom") return form.customWindow || null;
  const n = Number(form.windowChoice);
  return Number.isFinite(n) && n > 0 ? n : null;
});

/** Ollama /api/show 自动检测（读不到 → 错误条提示手动选择）。 */
async function probeWindow() {
  if (form.probing || !form.model.trim()) return;
  form.probing = true;
  form.probeError = "";
  try {
    const n = await llmProbeWindow(form.baseUrl, form.model);
    form.windowChoice = String(n);
  } catch (e) {
    form.probeError = String(e);
  } finally {
    form.probing = false;
  }
}

function save() {
  void settings.saveLlm({
    provider: form.provider,
    model: form.model,
    baseUrl: form.baseUrl, // 空串 = 清除，回落 provider 默认端点
    apiKey: form.apiKey || undefined, // 留空 = 不改动已存 Key
    windowTokens: formWindow.value, // null = 清除窗口（回落保守保护）
  });
  form.apiKey = "";
}

function clearKey() {
  void settings.saveLlm({ apiKey: "" });
}
</script>

<template>
  <section class="space-y-3 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.llmTitle") }}</h3>
    <p class="text-[11px] text-ink-4">{{ t("settings.llmHint") }}</p>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.provider") }}</span>
      <select v-model="form.provider" class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5">
        <option value="openai">OpenAI</option>
        <option value="anthropic">Anthropic</option>
        <option value="ollama">Ollama</option>
      </select>
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.model") }}</span>
      <input
        v-model="form.model"
        type="text"
        class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5"
      />
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.baseUrl") }}</span>
      <input
        v-model="form.baseUrl"
        type="text"
        :placeholder="t('settings.baseUrlPlaceholder')"
        class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 placeholder:text-ink-5"
      />
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.apiKey") }}</span>
      <input
        v-model="form.apiKey"
        type="password"
        autocomplete="off"
        :placeholder="settings.view?.llm.apiKeySet ? t('settings.apiKeyKeep') : t('settings.apiKeyPlaceholder')"
        class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 placeholder:text-ink-5"
      />
    </label>

    <!-- 上下文窗口（S2 迁入）：压缩阈值的换算基准，档位呈现（§3.5） -->
    <div class="space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.window") }}</span>
      <div class="flex items-center gap-2">
        <select
          v-model="form.windowChoice"
          class="block min-w-0 flex-1 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5"
        >
          <option value="unset">{{ t("settings.windowUnset") }}</option>
          <option v-for="p in PRESETS" :key="p" :value="String(p)">
            {{ compactNumber(p) }}
          </option>
          <option value="custom">{{ t("settings.windowCustom") }}</option>
        </select>
        <button
          v-if="form.provider === 'ollama'"
          class="shrink-0 rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-2 transition hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="form.probing"
          @click="probeWindow"
        >
          {{ form.probing ? t("settings.windowProbing") : t("settings.windowDetect") }}
        </button>
      </div>
      <p v-if="form.probeError" class="text-[11px] text-danger">
        {{ t("settings.windowDetectFail") }} {{ form.probeError }}
      </p>
    </div>
    <label v-if="form.windowChoice === 'custom'" class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.windowCustomLabel") }}</span>
      <input
        v-model.number="form.customWindow"
        type="number"
        min="100"
        step="1000"
        class="block w-full rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5"
      />
    </label>

    <div class="flex items-center gap-2">
      <button
        class="rounded-lg bg-wash-2 px-4 py-1.5 text-xs font-semibold transition hover:bg-wash-3 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="settings.saving"
        @click="save"
      >
        {{ t("settings.save") }}
      </button>
      <button
        v-if="settings.view?.llm.apiKeySet"
        class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-3 transition hover:bg-surface-1"
        :disabled="settings.saving"
        @click="clearKey"
      >
        {{ t("settings.clearKey") }}
      </button>
      <span v-if="settings.savedFlash" class="text-xs text-ok">
        {{ t("settings.saved") }}
      </span>
    </div>
  </section>
</template>
