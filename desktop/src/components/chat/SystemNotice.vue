<script setup lang="ts">
/**
 * 会话内系统消息模式（壳批次，docs/design/UI shell and patterns.md §3.3）：
 * 压缩通知 / "已记住：xxx" / 审批结果等"Agent 对会话状态的陈述"统一形态
 * ——非气泡、非工具卡，居中窄条 + 可选动作按钮。
 *
 * v2.143 发丝线形态（用户裁定，参考「上下文已压缩」分隔线）：hairline
 * 时渲染为一根贯穿列宽的发丝线 + 状态文字居中盖在线上（压缩发起/收束
 * 同一条线就地改写，见 chat store pushCompactNotice）。
 *
 * 动作按钮只上抛 action(id)（如 details / undo），撤销等
 * 实际行为由宿主决定（⑨ 接线 rewind_to 与详情面板）。文案是 UI 不是
 * 会话数据，由调用方传 i18n key 在渲染期翻译（跟随语言切换）。
 */
defineProps<{
  text: string;
  actions?: { id: string; label: string; tone?: "primary" | "ghost" }[];
  /** 发丝线分隔形态（v2.143）：缺省 = 居中卡片窄条。 */
  hairline?: boolean;
}>();

defineEmits<{ (e: "action", id: string): void }>();
</script>

<template>
  <!-- 入场动效 = §六动效②（淡入 + 上滑；appear 覆盖插入即挂载的场景） -->
  <Transition name="lw-notice" appear>
    <!-- 发丝线形态：两段线夹居中文字组（查看动作在线上文字旁，不打断对称） -->
    <div v-if="hairline" class="sys-hairline" role="status">
      <span class="sys-hairline__rule" aria-hidden="true" />
      <span class="sys-hairline__core">
        <span class="sys-hairline__label">{{ text }}</span>
        <button
          v-for="a in actions ?? []"
          :key="a.id"
          type="button"
          class="sys-hairline__btn"
          @click="$emit('action', a.id)"
        >
          {{ a.label }}
        </button>
      </span>
      <span class="sys-hairline__rule" aria-hidden="true" />
    </div>
    <div v-else class="flex justify-center">
      <div
        class="flex max-w-[85%] flex-wrap items-center gap-x-2 gap-y-1 rounded-[var(--lw-r-xs)] border border-line bg-wash-1 px-4 py-1.5 text-xs text-ink-3"
      >
        <span class="whitespace-pre-wrap">{{ text }}</span>
        <button
          v-for="a in actions ?? []"
          :key="a.id"
          class="rounded-full px-2 py-0.5 text-[length:var(--lw-fs-11)] font-medium transition"
          :class="
            a.tone === 'primary'
              ? 'bg-surface-2 text-ink-1 hover:bg-line-strong'
              : 'text-accent-hover hover:underline'
          "
          @click="$emit('action', a.id)"
        >
          {{ a.label }}
        </button>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
/* 系统条发丝线（v2.143）：发丝线两端不到头由转录列 padding 承载；文字组
 * 盖在线中断处，长文案（熔断）可折行不撑破。 */
.sys-hairline {
  display: flex;
  align-items: center;
  width: 100%;
  min-width: 0;
}
.sys-hairline__rule {
  flex: 1 1 0;
  height: 1px;
  background: var(--lw-line-subtle);
}
.sys-hairline__core {
  display: inline-flex;
  align-items: center;
  gap: var(--lw-gap-8);
  min-width: 0;
  max-width: 72%;
  padding: 0 var(--lw-gap-10);
}
.sys-hairline__label {
  min-width: 0;
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
  text-align: center;
}
.sys-hairline__btn {
  flex: none;
  font-size: var(--lw-fs-11);
  color: var(--lw-accent);
}
.sys-hairline__btn:hover {
  text-decoration: underline;
}
</style>
