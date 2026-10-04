<script setup lang="ts">
/**
 * 设置分区：上下文管理（settings.json `context` 区，⑨；Context and
 * Memory.md §3.8.1）。阈值滑条 10%–90%（10% 一级）双显示（百分比给
 * 判断、绝对 token 给感知）；建议值动态算；<20% 警告；策略三档。
 * 保存即生效（后端换入 ctx-compact 活配置）。
 * S2（清单 §5）：窗口下拉迁模型配置区——阈值为百分比，不需绝对窗口；
 * 预览换算的窗口读 llm 区（未设置回落 FALLBACK 保守保护）。
 * 行版式（§3.32·七 v3.5）：动态取值（当前值/建议/警告）走行左 #detail。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { compactNumber } from "../../lib/format";
import { FALLBACK_WINDOW_TOKENS, suggestedThreshold } from "../../lib/context";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSelect from "./SettingsSelect.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const STRATEGIES = ["sliding_window", "layered", "summary_only"] as const;

const strategyOptions = computed(() =>
  STRATEGIES.map((s) => ({
    value: s,
    label: t(`settings.strategy_${s}`),
    desc: t(`settings.strategy_${s}_desc`),
  })),
);

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
  <SettingsGroup :hint="t('settings.contextHint')">
    <!-- 压缩阈值滑条：滑条在右（wide 放开 240 上限），当前值/建议/警告
         走左列 #detail（§3.2.2 双显示，绝对数按 §3.5 档位） -->
    <SettingsRow :title="t('settings.threshold')" wide>
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">
          {{ t("settings.thresholdValue", { pct: form.thresholdPct, tokens: compactNumber(thresholdTokens) }) }}
        </p>
        <p v-if="suggestedPct != null" class="mt-0.5 text-[11px] text-ink-4">
          {{ t("settings.thresholdSuggest", { pct: suggestedPct }) }}
          <span class="text-ink-5">{{ t("settings.thresholdSuggestHint") }}</span>
        </p>
        <p v-if="form.thresholdPct < 20" class="mt-0.5 text-[11px] text-warn">
          ⚠ {{ t("settings.thresholdLowWarning") }}
        </p>
        <!-- M2：阈值滑条复用为记忆树整理触发（单旋钮裁定，树 v2 §4.1） -->
        <p class="mt-0.5 text-[11px] text-ink-4">
          {{ t("settings.thresholdOrganizeHint") }}
        </p>
      </template>
      <input
        v-model.number="form.thresholdPct"
        type="range"
        min="10"
        max="90"
        step="10"
        class="w-48 accent-[var(--lw-slider-accent)]"
      />
    </SettingsRow>

    <!-- 压缩策略三档（§3.3）：互斥多选一用下拉（§3.32·七 v3.5；
         v3.6 白底台面 + 白面板；v3.7 主名留触发器、说明进面板小字） -->
    <SettingsRow :title="t('settings.strategy')">
      <SettingsSelect
        v-model="form.strategy"
        :entries="strategyOptions"
        :label="t('settings.strategy')"
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
