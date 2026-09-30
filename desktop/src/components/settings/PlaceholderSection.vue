<script setup lang="ts">
/**
 * 设置分区占位（壳批次）：context（⑨ 上下文压缩）/ memory（⑪ 记忆注入）
 * / developer（⑫ 暴露调试面板）三个分区先占位锁定位置，字段随对应
 * 批次落到 settings.json 同名分区（分区一一对应规则见
 * docs/design/UI shell and patterns.md §3.2）。落位后删除占位、换成
 * 独立分区组件。
 */
import { useI18n } from "vue-i18n";

defineProps<{ titleKey: string; hintKey: string }>();

const { t } = useI18n();
</script>

<template>
  <section class="space-y-2 rounded-xl border border-dashed border-neutral-800 p-4">
    <h3 class="text-xs font-semibold">{{ t(titleKey) }}</h3>
    <p class="text-[11px] text-neutral-500">{{ t(hintKey) }}</p>
  </section>
</template>
