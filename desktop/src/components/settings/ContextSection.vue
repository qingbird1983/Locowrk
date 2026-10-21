<script setup lang="ts">
/**
 * 设置分区：上下文（S4 退役面 · specs/11 刀①；specs/12 刀② 加第四格）。
 *
 * 普通用户只负责四格：
 * ① 压缩阈值——档位滑条 100K–800K，每 100K 一档；到线自动压缩。
 * ② 自动压缩——开/关（关掉后手动 `/压缩` 仍可用）。
 * ③ 用量条显示——开/关。
 * ④ 摘要调用预算——三档预置（30s / 90s / 300s，缺省 90s）。只管**辅助路径**
 *    （自动压缩那次摘要能等多久），主对话轮的重试策略不受它影响。
 *
 * 其余数值全部写死进代码（输出预留 21K / 目标比 50% / 近段 15K / 工具输出 24 KiB）：
 * 它们没有"最优值"可由用户判断，抬上界面只是把实现参数变成负担。
 */
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import {
  SUMMARY_BUDGET_DEFAULT_MS,
  SUMMARY_BUDGET_PRESETS_MS,
  nearestSummaryBudget,
} from "../../lib/summary-budget";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsSegmented from "./SettingsSegmented.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const MIN = 100_000;
const MAX = 800_000;
const STEP = 100_000;

const form = reactive({
  thresholdTokens: 100_000,
  compactEnabled: true,
  usageBar: true,
  summaryBudgetMs: SUMMARY_BUDGET_DEFAULT_MS,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.thresholdTokens = v.context.thresholdTokens;
    form.compactEnabled = v.context.compactEnabled;
    form.usageBar = v.context.usageBar;
    form.summaryBudgetMs = v.context.summaryBudgetMs;
  },
  { immediate: true },
);

/** 气泡文本与说明行的档位文本（100K…800K）。 */
const thresholdText = computed(() => `${Math.round(form.thresholdTokens / 1000)}K`);

/** 预算档位（秒）→ 分段控件选项。三档预置，不给数字输入框。 */
const budgetOptions = computed(() =>
  SUMMARY_BUDGET_PRESETS_MS.map((ms) => ({
    value: String(ms),
    label: t("settings.summaryBudgetOption", { n: Math.round(ms / 1000) }),
  })),
);

/** 草稿里的毫秒数 ↔ 分段控件的字符串档位。
 *  非档值（老配置 / 手改过的 settings.json）就近显示但**不改存量**——点选才落档
 *  （同 `nearestUiFontPreset` 那条纪律）。 */
const budgetPreset = computed({
  get: () => String(nearestSummaryBudget(form.summaryBudgetMs)),
  set: (v: string) => {
    form.summaryBudgetMs = Number(v);
  },
});

// 气泡跟随：滑条拖动/悬停时显示，left 按值域百分比折算（thumb 半径用
// 固定补偿即可，800K 值域下误差 ≤ 半档；两端向内钳 10% 防出框）。
const dragging = ref(false);
const bubbleLeft = computed(() => {
  const pct = (form.thresholdTokens - MIN) / (MAX - MIN);
  return `${10 + pct * 80}%`;
});

function save() {
  void settings.saveContext({
    thresholdTokens: Math.round(form.thresholdTokens),
    compactEnabled: form.compactEnabled,
    usageBar: form.usageBar,
    summaryBudgetMs: form.summaryBudgetMs,
  });
}
</script>

<template>
  <SettingsGroup :hint="t('settings.contextHint')">
    <!-- 压缩阈值：唯一保留的用户数值，且是档位式（100K 一档），不是自由填数。 -->
    <SettingsRow :title="t('settings.threshold')" wide>
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">
          {{ t("settings.thresholdValue", { n: thresholdText }) }}
        </p>
        <p class="mt-0.5 text-[length:var(--lw-fs-11)] text-ink-4">
          {{ t("settings.thresholdHint") }}
        </p>
      </template>
      <div class="relative pt-7">
        <span
          v-show="dragging"
          class="pointer-events-none absolute top-0 -translate-x-1/2 rounded-[var(--lw-r-sm)] border border-line-subtle bg-elevated px-1.5 py-0.5 text-[length:var(--lw-fs-11)] text-ink-1 shadow-raised"
          :style="{ left: bubbleLeft }"
        >
          {{ thresholdText }}
        </span>
        <input
          v-model.number="form.thresholdTokens"
          type="range"
          :min="MIN"
          :max="MAX"
          :step="STEP"
          class="w-48 accent-[var(--lw-slider-accent)]"
          :aria-label="t('settings.threshold')"
          @pointerdown="dragging = true"
          @pointerup="dragging = false"
          @pointercancel="dragging = false"
          @blur="dragging = false"
        />
      </div>
    </SettingsRow>

    <!-- 自动压缩：到线 → 模型停在这一步 → 摘要回填 → 继续。 -->
    <SettingsRow :title="t('settings.autoCompact')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.autoCompactHint") }}</p>
      </template>
      <SettingsToggle v-model="form.compactEnabled" :label="t('settings.autoCompact')" />
    </SettingsRow>

    <!-- 摘要调用预算：三档预置（辅助路径的总时长上限），不给数字输入框。 -->
    <SettingsRow :title="t('settings.summaryBudget')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.summaryBudgetHint") }}</p>
      </template>
      <SettingsSegmented
        v-model="budgetPreset"
        :options="budgetOptions"
        :label="t('settings.summaryBudget')"
      />
    </SettingsRow>

    <!-- 用量条显隐。 -->
    <SettingsRow :title="t('settings.usageBar')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.usageBarHint") }}</p>
      </template>
      <SettingsToggle v-model="form.usageBar" :label="t('settings.usageBar')" />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
