<script setup lang="ts">
/**
 * 会话内系统消息模式（壳批次，docs/design/UI shell and patterns.md §3.3）：
 * 压缩通知 / "已记住：xxx" / 审批结果等"Agent 对会话状态的陈述"统一形态
 * ——非气泡、非工具卡，居中窄条 + 可选动作按钮。
 *
 * 动作按钮只上抛 action(id)（如 details / undo），跳转或撤销等实际行为
 * 由宿主决定（⑨ 接线 rewind_to 与详情面板）。文案是 UI 不是会话数据，
 * 由调用方传 i18n key 在渲染期翻译（跟随语言切换）。
 */
defineProps<{
  text: string;
  actions?: { id: string; label: string; tone?: "primary" | "ghost" }[];
}>();

defineEmits<{ (e: "action", id: string): void }>();
</script>

<template>
  <div class="flex justify-center">
    <div
      class="flex max-w-[85%] flex-wrap items-center gap-x-2 gap-y-1 rounded-lg border border-[var(--lw-border)] bg-[var(--lw-surface-1)]/60 px-4 py-1.5 text-xs text-[var(--lw-text-2)]"
    >
      <span class="whitespace-pre-wrap">{{ text }}</span>
      <button
        v-for="a in actions ?? []"
        :key="a.id"
        class="rounded-full px-2 py-0.5 text-[11px] font-medium transition"
        :class="
          a.tone === 'primary'
            ? 'bg-[var(--lw-surface-2)] text-[var(--lw-text-1)] hover:bg-[var(--lw-border-strong)]'
            : 'text-[var(--lw-accent-hover)] hover:underline'
        "
        @click="$emit('action', a.id)"
      >
        {{ a.label }}
      </button>
    </div>
  </div>
</template>
