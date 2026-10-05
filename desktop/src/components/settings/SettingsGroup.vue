<script setup lang="ts">
/**
 * 设置行分组（UI shell 文档 §3.32·七 v3.5）：可选 13px/600 小标题 +
 * 块下说明 + 行瓦片 gap 6px 纵向堆叠（行间空隙替代发丝线）。标题行右侧
 * #action 槽放块级动作（如「添加目录…」/「刷新」）。多块板面（如工具与
 * 暴露）用它分块；单块板面可不带标题。
 */
withDefaults(
  defineProps<{
    title?: string;
    hint?: string;
  }>(),
  { title: undefined, hint: undefined },
);
</script>

<template>
  <section class="space-y-2">
    <div v-if="title || hint || $slots.action" class="flex items-end justify-between gap-5 px-1">
      <div class="min-w-0">
        <h3 v-if="title" class="text-[length:var(--lw-fs-13)] font-semibold text-ink-1">{{ title }}</h3>
        <p v-if="hint" class="mt-0.5 text-[length:var(--lw-fs-12)] leading-relaxed text-ink-4">
          {{ hint }}
        </p>
      </div>
      <slot name="action" />
    </div>
    <div class="flex flex-col gap-1.5">
      <slot />
    </div>
  </section>
</template>
