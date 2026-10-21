<script setup lang="ts">
/**
 * 设置分区：上下文（S4 退役面 · specs/11 刀①；specs/12 刀② 加第四格；
 * specs/13 刀① 把「压缩阈值」从绝对 token 换成**占模型窗口的百分比**）。
 *
 * 普通用户只负责四格：
 * ① 自动压缩阈值——**占模型窗口的百分比**，三档预置（25% / 50% / 85%，缺省 50%）。
 *    为什么不是绝对 token：绝对数不跟窗口走（976K 窗口的模型按 100K 压 = 永远
 *    压不完，128K 窗口的模型按 100K 压 = 永远压不到），跨模型不可预期。
 * ② 自动压缩——开/关（关掉后手动 `/压缩` 仍可用，与上游同款）。
 * ③ 摘要调用预算——三档预置（30s / 90s / 300s，缺省 90s）。只管**辅助路径**
 *    （压缩那次摘要能等多久），主对话轮的重试策略不受它影响。
 * ④ 用量条显示——开/关。
 *
 * 其余数值全部写死进代码（输出预留 21K / 近段 15K / 工具输出 24 KiB）：
 * 它们没有"最优值"可由用户判断，抬上界面只是把实现参数变成负担。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import {
  AUTO_COMPACT_PCT_DEFAULT,
  AUTO_COMPACT_PCT_PRESETS,
  nearestAutoCompactPct,
} from "../../lib/auto-compact-pct";
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

const form = reactive({
  autoCompactPct: AUTO_COMPACT_PCT_DEFAULT,
  compactEnabled: true,
  usageBar: true,
  summaryBudgetMs: SUMMARY_BUDGET_DEFAULT_MS,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.autoCompactPct = v.context.autoCompactPct;
    form.compactEnabled = v.context.compactEnabled;
    form.usageBar = v.context.usageBar;
    form.summaryBudgetMs = v.context.summaryBudgetMs;
  },
  { immediate: true },
);

/** 阈值档位 → 分段控件选项。三档预置，不给数字输入框。 */
const pctOptions = computed(() =>
  AUTO_COMPACT_PCT_PRESETS.map((pct) => ({
    value: String(pct),
    label: t("settings.autoCompactPctOption", { n: pct }),
  })),
);

/** 草稿里的百分比 ↔ 分段控件的字符串档位。
 *  非档值（老配置 / 手改过的 settings.json）就近显示但**不改存量**——点选才落档
 *  （同 `nearestUiFontPreset` 那条纪律）。 */
const pctPreset = computed({
  get: () => String(nearestAutoCompactPct(form.autoCompactPct)),
  set: (v: string) => {
    form.autoCompactPct = Number(v);
  },
});

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

function save() {
  void settings.saveContext({
    autoCompactPct: Math.round(form.autoCompactPct),
    compactEnabled: form.compactEnabled,
    usageBar: form.usageBar,
    summaryBudgetMs: form.summaryBudgetMs,
  });
}
</script>

<template>
  <SettingsGroup :hint="t('settings.contextHint')">
    <!-- 自动压缩阈值：占模型窗口的百分比（不是绝对 token —— 绝对数不跟窗口走）。 -->
    <SettingsRow :title="t('settings.autoCompactPct')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.autoCompactPctHint") }}</p>
      </template>
      <SettingsSegmented
        v-model="pctPreset"
        :options="pctOptions"
        :label="t('settings.autoCompactPct')"
      />
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
