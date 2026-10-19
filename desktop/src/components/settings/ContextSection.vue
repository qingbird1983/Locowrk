<script setup lang="ts">
/**
 * 设置分区：上下文管理（settings.json `context` 区，⑨）。v2.137（用户
 * 裁定「上下文根本不压缩了」）：整理阈值（绝对 token，100K–800K 每 100K
 * 一档）——对话上下文达到该 token 数触发记忆树整理；旧「压缩阈值（百分比）
 * / 压缩策略 / 稳定期」随 ctx-compact 桌面链退役。Q8（CB §7.4）加第二根
 * 旋钮 = 工具结果预算（KiB，0 = 不限制）：它挡的是「一条工具输出灌进上下文」
 * 这件事，与阈值管「多久整理一次」是两回事，所以本区不再只有一根旋钮。
 * 滑动时气泡跟随显示当前档位（100K…800K）。保存即生效（下一轮触发判定
 * 读设置，runner UsagePolicy 同源；预算走 llm 组装点 → wire 发送侧）。
 * S2 刀③ 加第三根旋钮 = 微压缩开关（context.microEnabled，缺省开）：它管的不是
 * 「一条多大」（预算）也不是「多久整理」（阈值），而是「很旧的工具输出要不要
 * 先换成一行占位」——同一区各挡一件事。布尔一律开关（§3.32·七）。
 * 行版式（§3.32·七 v3.5）：说明文字走行左 #detail。
 */
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const MIN = 100_000;
const MAX = 800_000;
const STEP = 100_000;

const form = reactive({
  thresholdTokens: 100_000,
  usageBar: true,
  // Q8：工具结果预算（KiB，0 = 不限制）。原样传后端，前端不换算成字节
  toolResultBudgetKib: 24,
  // S2 刀③：微压缩开关（缺省开）。前端只传布尔，判据全在后端
  microEnabled: true,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.thresholdTokens = v.context.thresholdTokens;
    form.usageBar = v.context.usageBar;
    form.toolResultBudgetKib = v.context.toolResultBudgetKib;
    form.microEnabled = v.context.microEnabled;
  },
  { immediate: true },
);

/** 气泡文本与说明行的档位文本（100K…800K）。 */
const thresholdText = computed(() => `${Math.round(form.thresholdTokens / 1000)}K`);

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
    usageBar: form.usageBar,
    // 输入框被清空时 v-model.number 给 NaN → JSON 序列化成 null → 后端按
    // None 处理（保持不变，不是存 0）。0 只能来自用户明确填 0 = 不限制。
    toolResultBudgetKib: Math.round(form.toolResultBudgetKib),
    microEnabled: form.microEnabled,
  });
}
</script>

<template>
  <SettingsGroup :hint="t('settings.contextHint')">
    <!-- 整理阈值滑条（100K–800K，100K 一档）：滑动时气泡跟随显示当前
         档位；说明走左列 #detail（窗口红线 + 手动 /整理 不受限） -->
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

    <!-- Q8 工具结果预算（CB §7.4）：一条工具输出进上下文的上限，0 = 不限制。
         数值档与 Q4 的 instruction*Kib 同形（数字输入 + min/max，不换算字节） -->
    <SettingsRow
      :title="t('settings.toolBudgetKib')"
      :hint="t('settings.toolBudgetKibHint')"
      control-id="context-tool-budget-kib"
    >
      <input
        id="context-tool-budget-kib"
        v-model.number="form.toolResultBudgetKib"
        type="number"
        min="0"
        max="512"
        step="8"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- 微压缩开关（S2 刀③）：旧工具结果清成一行占位，先于整理触发。
         布尔项一律开关（§3.32·七），说明走 #detail 与进度条那行同形 -->
    <SettingsRow :title="t('settings.microCompact')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.microCompactHint") }}</p>
      </template>
      <SettingsToggle v-model="form.microEnabled" :label="t('settings.microCompact')" />
    </SettingsRow>

    <!-- 阈值用量进度条显隐（§3.4 v4.18；v2.137 改名——分母 = 整理阈值；
         布尔项一律开关 §3.32·七） -->
    <SettingsRow :title="t('settings.usageBar')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.usageBarHint") }}</p>
      </template>
      <SettingsToggle v-model="form.usageBar" :label="t('settings.usageBar')" />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
