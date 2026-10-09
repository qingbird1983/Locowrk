<script setup lang="ts">
/**
 * 设置分区：上下文管理（settings.json `context` 区，⑨）。v2.137（用户
 * 裁定「上下文根本不压缩了」）：唯一旋钮 = 整理阈值（绝对 token，
 * 100K–800K 每 100K 一档）——对话上下文达到该 token 数触发记忆树整理；
 * 旧「压缩阈值（百分比）/ 压缩策略 / 稳定期」随 ctx-compact 桌面链退役。
 * 滑动时气泡跟随显示当前档位（100K…800K）。保存即生效（下一轮触发判定
 * 读设置，runner UsagePolicy 同源）。
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
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.thresholdTokens = v.context.thresholdTokens;
    form.usageBar = v.context.usageBar;
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
