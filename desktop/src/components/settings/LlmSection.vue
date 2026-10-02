<script setup lang="ts">
/**
 * 设置分区：LLM 配置（settings.json `llm` 区，§13.1）。保存即生效，
 * API Key 只落盘不出 IPC——输入框永远从空开始，placeholder 按
 * apiKeySet 提示"留空保持不变"。
 * S2（清单 §5 信息归属）：模型上下文窗口是模型属性，下拉自上下文管理
 * 区迁入；Ollama 走 /api/show 自动检测，云端模型查静态档案兜底。
 * 行版式（§3.32·七 v3.5）：左行名右控件，文本输入 min 140 自适应。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { compactNumber } from "../../lib/format";
import { profileWindow } from "../../lib/model-profiles";
import { llmProbeWindow } from "../../lib/api";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";

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
  <SettingsGroup :hint="t('settings.llmHint')">
    <SettingsRow :title="t('settings.provider')" control-id="llm-provider">
      <select
        id="llm-provider"
        v-model="form.provider"
        class="w-44 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
      >
        <option value="openai">OpenAI</option>
        <option value="anthropic">Anthropic</option>
        <option value="ollama">Ollama</option>
      </select>
    </SettingsRow>

    <SettingsRow :title="t('settings.model')" control-id="llm-model">
      <input
        id="llm-model"
        v-model="form.model"
        type="text"
        class="w-full min-w-[140px] rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
      />
    </SettingsRow>

    <SettingsRow :title="t('settings.baseUrl')" control-id="llm-base-url">
      <input
        id="llm-base-url"
        v-model="form.baseUrl"
        type="text"
        :placeholder="t('settings.baseUrlPlaceholder')"
        class="w-full min-w-[140px] rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs placeholder:text-ink-5"
      />
    </SettingsRow>

    <SettingsRow :title="t('settings.apiKey')" control-id="llm-api-key">
      <input
        id="llm-api-key"
        v-model="form.apiKey"
        type="password"
        autocomplete="off"
        :placeholder="settings.view?.llm.apiKeySet ? t('settings.apiKeyKeep') : t('settings.apiKeyPlaceholder')"
        class="w-full min-w-[140px] rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs placeholder:text-ink-5"
      />
    </SettingsRow>

    <!-- 上下文窗口（S2 迁入）：压缩阈值的换算基准，档位呈现（§3.5）；
         Ollama 检测钮与下拉同行（控件列 gap-2） -->
    <SettingsRow :title="t('settings.window')" control-id="llm-window">
      <select
        id="llm-window"
        v-model="form.windowChoice"
        class="w-44 min-w-0 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
      >
        <option value="unset">{{ t("settings.windowUnset") }}</option>
        <option v-for="p in PRESETS" :key="p" :value="String(p)">
          {{ compactNumber(p) }}
        </option>
        <option value="custom">{{ t("settings.windowCustom") }}</option>
      </select>
      <button
        v-if="form.provider === 'ollama'"
        type="button"
        class="shrink-0 rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-2 transition hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="form.probing"
        @click="probeWindow"
      >
        {{ form.probing ? t("settings.windowProbing") : t("settings.windowDetect") }}
      </button>
    </SettingsRow>
    <SettingsRow
      v-if="form.windowChoice === 'custom'"
      :title="t('settings.windowCustomLabel')"
      control-id="llm-window-custom"
    >
      <input
        id="llm-window-custom"
        v-model.number="form.customWindow"
        type="number"
        min="100"
        step="1000"
        class="w-[140px] rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
      />
    </SettingsRow>
    <p v-if="form.probeError" class="px-1 text-[11px] text-danger">
      {{ t("settings.windowDetectFail") }} {{ form.probeError }}
    </p>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save">
    <button
      v-if="settings.view?.llm.apiKeySet"
      type="button"
      class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-3 transition hover:bg-surface-1"
      :disabled="settings.saving"
      @click="clearKey"
    >
      {{ t("settings.clearKey") }}
    </button>
  </SettingsSaveRow>
</template>
