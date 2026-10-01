<script setup lang="ts">
/**
 * 设置分区：上下文管理（settings.json `context` 区，⑨；Context and
 * Memory.md §3.8.1）。阈值滑条 10%–90%（10% 一级）双显示（百分比给
 * 判断、绝对 token 给感知）；建议值动态算；<20% 警告；策略三档。
 * 保存即生效（后端换入 ctx-compact 活配置）。
 * S2（清单 §5）：窗口下拉迁模型配置区——阈值为百分比，不需绝对窗口；
 * 预览换算的窗口读 llm 区（未设置回落 FALLBACK 保守保护）。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { compactNumber } from "../../lib/format";
import { FALLBACK_WINDOW_TOKENS, suggestedThreshold } from "../../lib/context";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const STRATEGIES = ["sliding_window", "layered", "summary_only"] as const;

// 表单是投影的本地可编辑副本（S2：只含阈值 + 策略）
const form = reactive({
  thresholdPct: 30,
  strategy: "layered",
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.thresholdPct = v.context.thresholdPct;
    form.strategy = v.context.strategy;
  },
  { immediate: true },
);

/** 生效窗口（llm 区为准；未设置回落保守保护，仅用于预览换算）。 */
const formWindow = computed<number | null>(() => settings.view?.llm.windowTokens ?? null);

const thresholdTokens = computed(
  () => ((formWindow.value ?? FALLBACK_WINDOW_TOKENS) * form.thresholdPct) / 100,
);

const suggestedPct = computed(() =>
  formWindow.value == null ? null : suggestedThreshold(formWindow.value),
);

function save() {
  void settings.saveContext({
    thresholdPct: form.thresholdPct,
    strategy: form.strategy,
  });
}
</script>

<template>
  <section class="space-y-3 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.contextTitle") }}</h3>
    <p class="text-[11px] text-ink-4">{{ t("settings.contextHint") }}</p>

    <!-- 压缩阈值滑条：百分比与绝对 token 双显示（§3.2.2）；绝对数按
         §3.5 档位（≈51.2K），不再露出 52,428.8 这类机器精度 -->
    <div class="space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.threshold") }}</span>
      <input
        v-model.number="form.thresholdPct"
        type="range"
        min="10"
        max="90"
        step="10"
        class="w-full accent-[var(--lw-accent)]"
      />
      <p class="text-ink-3">
        {{ t("settings.thresholdValue", { pct: form.thresholdPct, tokens: compactNumber(thresholdTokens) }) }}
      </p>
      <p v-if="suggestedPct != null" class="text-[11px] text-ink-4">
        {{ t("settings.thresholdSuggest", { pct: suggestedPct }) }}
        <span class="text-ink-5">{{ t("settings.thresholdSuggestHint") }}</span>
      </p>
      <p
        v-if="form.thresholdPct < 20"
        class="text-[11px] text-warn"
      >
        ⚠ {{ t("settings.thresholdLowWarning") }}
      </p>
    </div>

    <!-- 压缩策略三档（§3.3） -->
    <div class="space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.strategy") }}</span>
      <label
        v-for="s in STRATEGIES"
        :key="s"
        class="flex cursor-pointer items-center gap-2 py-0.5"
      >
        <input v-model="form.strategy" type="radio" :value="s" class="accent-[var(--lw-accent)]" />
        <span>{{ t(`settings.strategy_${s}`) }}</span>
      </label>
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
  </section>
</template>
