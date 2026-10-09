<script setup lang="ts">
/**
 * 阈值用量进度条（壳批次 §3.4；v2.137 改名——分母 = 整理阈值非窗口）：
 * ⑨ 起 chat store 的 contextUsage 有数据；null = 无数据不渲染（骨架
 * 阶段恒隐藏，挂载点已在会话视图顶部就位）。
 *
 * 分级：绿 <60% / 黄 60–99% / 红 ≥100%（到整理阈值）。S2（§3.5 数字
 * 呈现）：右侧增 `≈52K / 256K` 比例段（分母 = 模型窗口，四层链自动
 * 识别）——窗口未知（0）时只显示百分比。高度 3px（v4.18 用户裁定，
 * 原 4px 收细一档）；显隐开关 context.usageBar 在挂载点裁决（App.vue）。
 */
import { computed } from "vue";
import { compactNumber } from "../lib/format";

const props = defineProps<{
  usage: { used: number; threshold: number; windowTokens: number } | null;
}>();

const pct = computed(() => {
  if (!props.usage || props.usage.threshold <= 0) return 0;
  return Math.min(100, (props.usage.used / props.usage.threshold) * 100);
});

const tone = computed(() => {
  if (pct.value >= 100) return "bg-danger";
  if (pct.value >= 60) return "bg-warn";
  return "bg-ok";
});

/** 比例段：`≈52K / 256K`；窗口未知 = 空串（只显示百分比）。 */
const ratio = computed(() => {
  if (!props.usage || props.usage.windowTokens <= 0) return "";
  return `${compactNumber(props.usage.used)} / ${compactNumber(props.usage.windowTokens)}`;
});
</script>

<template>
  <div v-if="usage" class="flex items-center gap-2 px-5 pt-2 text-[length:var(--lw-fs-10)]">
    <div
      class="h-[3px] min-w-0 flex-1 overflow-hidden rounded-full bg-surface-1"
    >
      <div class="lw-bar-fill h-full rounded-full" :class="tone" :style="{ width: `${pct}%` }" />
    </div>
    <span v-if="ratio" class="shrink-0 text-ink-4">
      {{ ratio }}
    </span>
    <span class="shrink-0 text-ink-4">
      {{ Math.round(pct) }}%
    </span>
  </div>
</template>
