<script setup lang="ts">
/**
 * 画布节点（队列 Q14 第三刀 · §二⑯「节点」行）。
 *
 * 三类节点（总结 / 标题 / 章节）**共用这一个构件**——§二⑯ 的原话是「节点就是上位
 * §八 标准内容卡那一套，不许另造」：底 `--lw-wash-1`、围边 `--lw-line-subtle` 一档、
 * 圆角 `--lw-r-md`（画布不计档 ⇒ 节点是第一个圆角容器）、卡内小块降 `--lw-r-xs`、
 * 横向内边距 16、hover 只走 §三 的 wash-2。层级不靠彩色分类，靠**左边色条浓度 +
 * 字号递减**两把尺，且两把尺都在这一处定义，三个调用方不各写一遍。
 *
 * ⚠ 宽度取 `NODE_W`（lib/board-model）而不是这里写死的数：连线端点几何必须可预测，
 * 定值散进 CSS 里缩放一极端就脱节（§二⑯「节点定宽的唯一理由」）。
 */
import { computed } from "vue";
import { NODE_W, type BoardVariant } from "../../../lib/board-model";

const props = withDefaults(
  defineProps<{
    variant: BoardVariant;
    /** 主字（总结正文 / 章节标题）。 */
    label: string;
    /** 次级正文（章节的 L1 摘要）；标题条不给。 */
    body?: string;
    /** 元信息行（时间 · 事件区间）。 */
    meta?: string;
    /** 已展开 = §三 的选中档 wash-3。 */
    open?: boolean;
    /** 不可点（占位桩「整理中…」，§2.2 第 2 条）。 */
    disabled?: boolean;
    /** 弱化档：未归总结虚拟组（§2.2 第 1 条「样式弱化，行为一致」）。 */
    muted?: boolean;
    /** 定高（仅汇聚面板用：高度 = 该总结子树的纵向跨度，由测量给，见
     * `BoardCanvas`；第 1–3 列一律 auto = 内容由字号与行数决定）。 */
    heightPx?: number;
  }>(),
  { body: "", meta: "", open: false, disabled: false, muted: false, heightPx: 0 },
);

const widthPx = computed(() => `${NODE_W[props.variant]}px`);
/** 定高只在给了数的时候写进内联样式——写 `height: 0` 会把面板压成一条。 */
const flowStyle = computed(() =>
  props.heightPx > 0 ? { height: `${props.heightPx}px` } : {},
);
</script>

<template>
  <!-- 汇聚面板必须是 div：里面是一整块可滚动内容，套在 button 里既点不准也
       读不成（无障碍树上会出现"一个能点的按钮，内含几十段正文"）。 -->
  <component
    :is="disabled || $slots.flow ? 'div' : 'button'"
    class="board-node"
    :class="[
      `is-${variant}`,
      { 'is-open': open, 'is-muted': muted, 'is-static': disabled, 'is-flow': $slots.flow },
    ]"
    :style="{ width: widthPx, ...flowStyle }"
    :type="disabled || $slots.flow ? undefined : 'button'"
    :disabled="disabled"
  >
    <!-- 汇聚面板：外壳与节点同形同档（§二⑯「汇聚面板」行），内容交给调用方的
         flow 槽——同一只壳件同时管四类节点，不在画布里再造第二个"面板壳"。 -->
    <slot v-if="$slots.flow" name="flow" />
    <template v-else>
      <span class="board-node__label">{{ label }}</span>
      <p v-if="body" class="board-node__body">{{ body }}</p>
      <span v-if="meta" class="board-node__meta">{{ meta }}</span>
      <span v-if="$slots.badges" class="board-node__badges"><slot name="badges" /></span>
    </template>
  </component>
</template>

<style scoped>
/* 围边只许 line-subtle 一档（§二⑯ 不授权清单：第二种围边色 / hover 变描边判错）。
   层级 = 左边色条三档浓度递减：line-strong（总结）→ line（标题）→ line-subtle（章节）。 */
.board-node {
  --lw-node-bar: var(--lw-line);
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-4);
  align-items: flex-start;
  box-sizing: border-box;
  padding: var(--lw-gap-10) var(--lw-gap-16);
  text-align: left;
  border: 1px solid var(--lw-line-subtle);
  border-radius: var(--lw-r-md);
  background: var(--lw-wash-1);
  transition: background-color var(--dur-fast) var(--ease-standard);
}
.board-node::before {
  content: "";
  position: absolute;
  inset-block: var(--lw-gap-10);
  left: 0;
  /* 光学修正：3px = 22px 网格上"看得见但不成第二套色档"的最细浓度条；
     4px 起会读成色块，把 §二⑯ 要的"浓度递减"压成三色板（§二⑬ 孤值凭证） */
  width: 3px;
  border-radius: var(--lw-r-3xs);
  background: var(--lw-node-bar);
}
.board-node.is-summary {
  --lw-node-bar: var(--lw-line-strong);
}
.board-node.is-title {
  --lw-node-bar: var(--lw-line);
}
.board-node.is-chapter {
  --lw-node-bar: var(--lw-line-subtle);
}
.board-node.is-static {
  cursor: default;
}
.board-node:not(.is-static):hover {
  background: var(--lw-wash-2);
}
.board-node.is-open {
  background: var(--lw-wash-3);
}
/* 未归总结虚拟组：主字降一个色档、色条降到最淡一档——形状一个字不改。 */
.board-node.is-muted {
  --lw-node-bar: var(--lw-line-subtle);
}
.board-node.is-muted .board-node__label {
  color: var(--lw-ink-3);
}
.board-node__label {
  font-size: var(--lw-fs-ui-base);
  color: var(--lw-ink-1);
  overflow: hidden;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 4;
}
.board-node__body {
  font-size: var(--lw-fs-ui-caption);
  color: var(--lw-ink-3);
  overflow: hidden;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 4;
}
.board-node__meta {
  font-size: var(--lw-fs-ui-xs);
  color: var(--lw-ink-4);
}
.board-node__badges {
  display: flex;
  gap: var(--lw-gap-4);
  align-items: center;
}
/* 标题列 = 窄条：只留主字一行半，几何不另造一档圆角或色档 */
.board-node.is-title {
  padding: var(--lw-gap-6) var(--lw-gap-16);
}
.board-node.is-title .board-node__label {
  font-size: var(--lw-fs-ui-caption);
  -webkit-line-clamp: 2;
}
/* 汇聚面板 = 同一只壳，内容自己滚（§二⑯「汇聚面板」行：外壳与节点同形同档，
   不新开一档）。hover 不给 wash-2：面板不是可点件，给了会读成"整块可点"。 */
.board-node.is-flow {
  cursor: default;
  display: block;
  overflow: hidden;
}
</style>
