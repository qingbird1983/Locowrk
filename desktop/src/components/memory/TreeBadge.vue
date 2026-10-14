<script setup lang="ts">
/**
 * 章节徽章（队列 Q14 第三刀 · §二⑧ 唯一公式的树侧落点）。
 *
 * 为什么提成一件：§二⑯「汇聚面板」行写的是「面板内部行不许自造，一律复用上位 §八
 * 既有形态」——画布的**小节头**要有来源徽章与「手动」徽章，而 TreeSection 里这四个
 * 徽章位（来源 / 手动标题 / 手动摘要 / 占位）原本是各自 inline 一段 span。抄进画布
 * 就是第五套；提成一件、两边共用才是「同类只许一种形态」。
 *
 * 形态 = §二⑧：胶囊 + 状态色 16% 底同色字 / 中性 = wash-3 底 + 次字色。字号沿用
 * 树侧既有的 `--lw-fs-10`（存量绝对档迁移是上位附录 C #2 第二半、队列 Q14 行内
 * 登记的独立待办，不在这一件里顺手改尺）。
 */
withDefaults(
  defineProps<{
    text: string;
    /** warn = 规则切分 / 占位这类降级产物；neutral = 记忆管理器与手动标记。 */
    tone?: "neutral" | "warn";
    /** 无障碍 / 悬停说明（如后端原始 source 串）。 */
    title?: string;
  }>(),
  { tone: "neutral", title: "" },
);
</script>

<template>
  <span
    class="shrink-0 rounded-full px-2 py-0.5 text-[length:var(--lw-fs-10)] leading-none"
    :class="tone === 'warn' ? 'bg-warn/16 text-warn' : 'bg-wash-3 text-ink-4'"
    :title="title || undefined"
  >
    {{ text }}
  </span>
</template>
