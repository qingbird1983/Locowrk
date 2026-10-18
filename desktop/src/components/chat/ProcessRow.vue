<script setup lang="ts">
/**
 * 过程行共享行壳（《Conversation Flow Rendering Spec v1.md》§6.2）：
 * flex 顺排六段 = 图标(14) / 类型词 / 摘要(1fr) / 元信息 / 状态 / 三角，
 * 思考行（§3.9）与工具行（§3.8）同吃一条行壳。
 *
 * 从 v4.13 五列栅格（20/38/1fr/80/24）退役而来：栅格能保证跨行同列对齐，
 * 但代价是类型词被定宽（最宽「子代理」≈37.5px 倒推），加不进语言徽章
 * 也换不了摘要形态；设计稿 §附录 A 取 flex（Claude 与 ZCode 都是 flex）。
 *
 * 图标位恒为**单色语义字形**（行首的铅笔 / 放大镜 / 终端）。语言 logo 不在这
 * 一段——它跟在文件名的前面（用户 2026-10-18 参考图裁定：两者并存，不是二选一），
 * 由消费方在 `#desc` 槽里放 `FileIcon`。
 *
 * 行 = 真 <button>（aria-expanded、键盘可达）；三角恒占位，行 hover / 键盘聚焦
 * 淡入（悬停不位移）；`foldable === false` 的身份（notify / browser_open /
 * dev_echo）三角 `aria-disabled` 且不响应点击。明细体不在本组件——展开只是
 * 追加缩进子行，各消费方自己 v-show。
 */
import { Icon, type IconName } from "../../lib/icons";

const props = defineProps<{
  icon: IconName;
  /** 类型词（i18n 后缀由调用方翻好传入）。 */
  type: string;
  expanded?: boolean;
  /** 缺省 true；false = 该身份不可折，三角占位但不响应（设计稿 §5.3）。 */
  foldable?: boolean;
  /** 展开整行的原生 title（描述省略时的完整摘要）。 */
  title?: string;
}>();

const emit = defineEmits<{ toggle: [] }>();

function onClick() {
  if (props.foldable === false) return;
  emit("toggle");
}
</script>

<template>
  <button
    class="prow"
    type="button"
    :aria-expanded="expanded ?? false"
    :title="title"
    @click="onClick"
  >
    <span class="prow__icon"><Icon :name="icon" :size="14" /></span>
    <span class="prow__type">{{ type }}</span>
    <span class="prow__desc">
      <span class="prow__name"><slot name="desc" /></span>
      <span v-if="$slots.tail" class="prow__tail"><slot name="tail" /></span>
    </span>
    <span v-if="$slots.meta" class="prow__meta"><slot name="meta" /></span>
    <span v-if="$slots.status" class="prow__status"><slot name="status" /></span>
    <span class="prow__chev" :aria-disabled="foldable === false || undefined">
      <Icon :name="expanded ? 'chevronDown' : 'chevronRight'" :size="14" />
    </span>
  </button>
</template>

<style scoped>
.prow {
  /* flex 六段（§6.2）：段宽随内容，摘要吃剩余 */
  display: flex;
  align-items: center;
  gap: var(--lw-gap-6);
  width: calc(100% + 6px);
  margin-inline: -3px;
  padding: 4px 3px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值4/3,§二⑬) */
  border-radius: var(--lw-r-xs);
  font-size: var(--lw-fs-ui-base);
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
  flex: none;
  color: var(--lw-ink-2);
}
.prow__type {
  flex: none;
  color: var(--lw-ink-3);
  font-size: var(--lw-fs-ui-sm);
  font-weight: 500;
  white-space: nowrap;
}
.prow__desc {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-4);
  flex: 1 1 auto;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
}
.prow__name {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--lw-ink-2);
}
.prow__tail {
  flex: none;
  color: var(--lw-ink-4);
  font-size: var(--lw-fs-ui-sm);
}
.prow__meta {
  flex: none;
  color: var(--lw-ink-3);
  font-size: var(--lw-fs-ui-sm);
  font-variant-numeric: tabular-nums;
}
.prow__status {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 5px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值5,§二⑬) */
  flex: none;
  white-space: nowrap;
  font-size: var(--lw-fs-ui-sm);
  color: var(--lw-ink-2);
}
/* 展开三角：恒占位，hover/键盘聚焦才显形（§3.8 红线：不位移） */
.prow__chev {
  display: inline-flex;
  align-items: center;
  flex: none;
  color: var(--lw-ink-3);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
.prow:hover .prow__chev,
.prow:focus-visible .prow__chev {
  opacity: 1;
}
</style>
