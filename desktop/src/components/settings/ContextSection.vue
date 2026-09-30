<script setup lang="ts">
/**
 * 设置分区：上下文管理（settings.json `context` 区，⑨；Context and
 * Memory.md §3.8.1）。窗口预设 + 自定义；阈值滑条 10%–90%（10% 一级）
 * 双显示（百分比给判断、绝对 token 给感知）；建议值动态算；<20% 警告；
 * 策略三档。保存即生效（后端换入 ctx-compact 活配置）。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FALLBACK_WINDOW_TOKENS, suggestedThreshold } from "../../lib/context";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const PRESETS = [8192, 16384, 32768, 65536, 131072, 200000, 262144, 1048576];
const STRATEGIES = ["sliding_window", "layered", "summary_only"] as const;

// 表单是投影的本地可编辑副本；窗口用"选项 + 自定义"两段表达
const form = reactive({
  windowChoice: "unset" as string, // "unset" | preset 数字串 | "custom"
  customWindow: 32768,
  thresholdPct: 30,
  strategy: "layered",
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    const w = v.context.windowTokens;
    form.windowChoice = w == null ? "unset" : String(w);
    form.thresholdPct = v.context.thresholdPct;
    form.strategy = v.context.strategy;
  },
  { immediate: true },
);

/** 表单当前生效的窗口（null = 未设置，回落保守保护）。 */
const formWindow = computed<number | null>(() => {
  if (form.windowChoice === "unset") return null;
  if (form.windowChoice === "custom") return form.customWindow || null;
  const n = Number(form.windowChoice);
  return Number.isFinite(n) && n > 0 ? n : null;
});

const thresholdTokens = computed(
  () => ((formWindow.value ?? FALLBACK_WINDOW_TOKENS) * form.thresholdPct) / 100,
);

const suggestedPct = computed(() =>
  formWindow.value == null ? null : suggestedThreshold(formWindow.value),
);

function save() {
  void settings.saveContext({
    windowTokens: formWindow.value, // null = 清除窗口（回落保守保护）
    thresholdPct: form.thresholdPct,
    strategy: form.strategy,
  });
}
</script>

<template>
  <section class="space-y-3 rounded-xl border border-neutral-800 p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.contextTitle") }}</h3>
    <p class="text-[11px] text-neutral-500">{{ t("settings.contextHint") }}</p>

    <!-- 模型上下文窗口：压缩阈值的换算基准（genai 不暴露，用户选择） -->
    <label class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.window") }}</span>
      <select
        v-model="form.windowChoice"
        class="block w-full rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5"
      >
        <option value="unset">{{ t("settings.windowUnset") }}</option>
        <option v-for="p in PRESETS" :key="p" :value="String(p)">
          {{ p.toLocaleString() }} token
        </option>
        <option value="custom">{{ t("settings.windowCustom") }}</option>
      </select>
    </label>
    <label v-if="form.windowChoice === 'custom'" class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.windowCustomLabel") }}</span>
      <input
        v-model.number="form.customWindow"
        type="number"
        min="100"
        step="1000"
        class="block w-full rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5"
      />
    </label>

    <!-- 压缩阈值滑条：百分比与绝对 token 双显示（§3.2.2） -->
    <div class="space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.threshold") }}</span>
      <input
        v-model.number="form.thresholdPct"
        type="range"
        min="10"
        max="90"
        step="10"
        class="w-full accent-emerald-500"
      />
      <p class="text-[var(--lw-text-2)]">
        {{ t("settings.thresholdValue", { pct: form.thresholdPct, tokens: thresholdTokens.toLocaleString() }) }}
      </p>
      <p v-if="suggestedPct != null" class="text-[11px] text-neutral-500">
        {{ t("settings.thresholdSuggest", { pct: suggestedPct }) }}
        <span class="text-neutral-600">{{ t("settings.thresholdSuggestHint") }}</span>
      </p>
      <p
        v-if="form.thresholdPct < 20"
        class="text-[11px] text-amber-400"
      >
        ⚠ {{ t("settings.thresholdLowWarning") }}
      </p>
    </div>

    <!-- 压缩策略三档（§3.3） -->
    <div class="space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.strategy") }}</span>
      <label
        v-for="s in STRATEGIES"
        :key="s"
        class="flex cursor-pointer items-center gap-2 py-0.5"
      >
        <input v-model="form.strategy" type="radio" :value="s" class="accent-emerald-500" />
        <span>{{ t(`settings.strategy_${s}`) }}</span>
      </label>
    </div>

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
  </section>
</template>
