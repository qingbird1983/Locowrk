<script setup lang="ts">
/**
 * 过程行共享五列栅格（UI shell doc §3.8 v4.13，《Tool Execution Process
 * Display》：图标 20px / 类型 38px（v4.15 用户目测定案，原文 64px；最宽
 * 类型词「子代理」三字 ≈37.5px 恰容纳）/ 描述 1fr / 状态 80px / 展开三角 24px，
 * CSS Grid 实现——思考行（§3.9）与工具行（§3.8）同吃一套列轨，所有行的
 * 同列严格对齐。行 = 真 <button>（aria-expanded、键盘可达）；三角默认
 * opacity 0，行 hover / 键盘聚焦淡入且列恒占位（悬停不位移）；描述单行
 * 省略；状态右对齐。明细体不在本组件——行展开只是追加缩进子行（各消费
 * 方自己 v-show，竖线层级在消费方），展开不换布局。
 */
import { Icon, type IconName } from "../../lib/icons";

defineProps<{
  icon: IconName;
  /** 类型词（i18n 后缀由调用方翻好传入）。 */
  type: string;
  expanded?: boolean;
  /** 展开整行的原生 title（描述省略时的完整摘要）。 */
  title?: string;
}>();

defineEmits<{ toggle: [] }>();
</script>

<template>
  <button
    class="prow"
    type="button"
    :aria-expanded="expanded ?? false"
    :title="title"
    @click="$emit('toggle')"
  >
    <span class="prow__icon"><Icon :name="icon" :size="14" /></span>
    <span class="prow__type">{{ type }}</span>
    <span class="prow__desc"><slot name="desc" /></span>
    <span class="prow__status"><slot name="status" /></span>
    <span class="prow__chev">
      <Icon :name="expanded ? 'chevronDown' : 'chevronRight'" :size="12" />
    </span>
  </button>
</template>

<style scoped>
.prow {
  display: grid;
  /* 五列栅格（§3.8 表）：图标/类型/描述/状态/三角——列宽恒定即全表对齐 */
  grid-template-columns: 20px 38px minmax(0, 1fr) 80px 24px;
  align-items: center;
  column-gap: var(--lw-gap-6);
  width: calc(100% + 6px);
  margin-inline: -3px;
  padding: 4px 3px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值4/3,§二⑬) */
  border-radius: var(--lw-r-xs);
  font-size: var(--lw-fs-12half);
  line-height: 1.3;
  color: var(--lw-ink-2);
  text-align: left;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.prow:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.prow__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* v4.14 第 2 层统一次要色：文字与图标同色，不再分层灰 */
  color: var(--lw-ink-2);
}
.prow__type {
  color: var(--lw-ink-2);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.prow__desc {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lw-ink-2);
}
.prow__status {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 5px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值5,§二⑬) */
  min-width: 0;
  white-space: nowrap;
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-2);
}
/* 展开三角：恒占位 24px 列，hover/键盘聚焦才显形（§3.8 红线：不位移） */
.prow__chev {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  color: var(--lw-ink-2);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
.prow:hover .prow__chev,
.prow:focus-visible .prow__chev {
  opacity: 1;
}
</style>
