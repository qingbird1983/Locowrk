<script setup lang="ts">
/**
 * 上下文进度条（壳批次，docs/design/UI shell and patterns.md §3.4）：
 * ⑨ 起 chat store 的 contextUsage 有数据；null = 无数据不渲染（骨架
 * 阶段恒隐藏，挂载点已在会话视图顶部就位）。
 *
 * 分级（Context and Memory.md §3.2.2）：绿 <60% / 黄 60–99% /
 * 红 ≥100%（到压缩阈值）。
 */
import { computed } from "vue";

const props = defineProps<{
  usage: { used: number; threshold: number } | null;
}>();

const pct = computed(() => {
  if (!props.usage || props.usage.threshold <= 0) return 0;
  return Math.min(100, (props.usage.used / props.usage.threshold) * 100);
});

const tone = computed(() => {
  if (pct.value >= 100) return "bg-red-500";
  if (pct.value >= 60) return "bg-amber-500";
  return "bg-emerald-500";
});
</script>

<template>
  <div v-if="usage" class="flex items-center gap-2 px-5 pt-2 text-[10px]">
    <div
      class="h-1 min-w-0 flex-1 overflow-hidden rounded-full bg-[var(--lw-surface-1)]"
    >
      <div
        class="h-full rounded-full transition-all"
        :class="tone"
        :style="{ width: `${pct}%` }"
      />
    </div>
    <span class="shrink-0 text-[var(--lw-text-3)]">
      {{ Math.round(pct) }}%
    </span>
  </div>
</template>
