<script setup lang="ts">
/**
 * 悬停说明气泡（前端标准 §二⑮ 唯一 tooltip 形态；原生 title 全站退役）：
 * 形态 = r-sm(10) + elevated 底 + 1px 淡发丝线 + 台面影 + 11px 次字色；
 * 时序唯一 = 悬停 400ms 延迟显示、移出立即消失（无退场渐隐）。
 * Teleport 挂 body + 量实际尺寸后定位（居中锚点、四边钳 8px、上下翻转）
 * ——绝对定位会被 overflow 祖先裁剪，必须出文档流。
 * 两种用法：①警示图标（icon，设置行旧用法）——触发件 = 内置 ⚠，默认插槽
 * = 气泡内容；②包裹任意触发件（icon 缺省 false）——默认插槽 = 触发件，
 * text = 气泡内容。label 供无障碍名（aria-label）。
 */
import { nextTick, ref, useSlots } from "vue";
import { Icon } from "../../lib/icons";

// 双根组件（锚点 span + Teleport 气泡）：Vue 对多根不做属性透传，须显式
// inheritAttrs:false + 锚点上 v-bind="$attrs"，调用方迁移来的布局类
// （w-full/flex-1/min-w-0/shrink-0…）才能落在包裹层——批③ 全站替换依赖此语义
defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    /** 气泡内容（icon=false 时与默认插槽分工：插槽=触发件，text=内容）。 */
    text?: string;
    /** 无障碍名。 */
    label?: string;
    /** true = 触发件是内置 ⚠ 警示图标（设置行旧用法），插槽 = 气泡内容。 */
    icon?: boolean;
    /** 弹出方位，缺省 top；放不下自动翻转。 */
    placement?: "top" | "bottom";
  }>(),
  { placement: "top" },
);

const slots = useSlots();
const anchor = ref<HTMLElement | null>(null);
const tipEl = ref<HTMLElement | null>(null);
const visible = ref(false);
const pos = ref({ left: -9999, top: -9999 });
let timer: number | null = null;

function show(): void {
  // 条件内容（原 :title="x || undefined" 一类）：无内容不弹空气泡
  if (!props.text && !(props.icon && slots.default)) return;
  if (timer !== null) window.clearTimeout(timer);
  // 悬停 400ms 才显示（§二⑮ 唯一时序）
  timer = window.setTimeout(() => {
    timer = null;
    void place();
  }, 400);
}
function hide(): void {
  if (timer !== null) {
    window.clearTimeout(timer);
    timer = null;
  }
  visible.value = false; // 移出立即消失，无渐隐
}

async function place(): Promise<void> {
  const a = anchor.value;
  if (!a) return;
  visible.value = true;
  await nextTick();
  const tip = tipEl.value;
  if (!tip) return;
  const r = a.getBoundingClientRect();
  const w = tip.offsetWidth;
  const h = tip.offsetHeight;
  let left = Math.max(8, Math.min(r.left + r.width / 2 - w / 2, window.innerWidth - w - 8));
  let top =
    props.placement === "top" ? r.top - h - 6 : r.bottom + 6;
  // 放不下向另一侧翻（贴边验收：§二③ 同款纪律）
  if (props.placement === "top" && top < 8) top = r.bottom + 6;
  if (props.placement === "bottom" && top + h > window.innerHeight - 8) top = r.top - h - 6;
  pos.value = { left: Math.round(left), top: Math.round(top) };
}
</script>

<template>
  <!-- 锚点 span 不设 position（气泡是 Teleport+fixed，不依赖它）——触发件
       自带 fixed/absolute（如钉窗按钮）时定位语义不被破坏，测量其真实位置 -->
  <span
    ref="anchor"
    v-bind="$attrs"
    class="inline-flex"
    role="note"
    :aria-label="label"
    @mouseenter="show"
    @mouseleave="hide"
    @focusin="show"
    @focusout="hide"
  >
    <Icon v-if="icon" name="warn" :size="13" class="text-warn" aria-hidden="true" />
    <slot v-else />
  </span>
  <Teleport to="body">
    <Transition name="lw-tip">
      <span
        v-if="visible"
        ref="tipEl"
        class="lw-tip"
        :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
        aria-hidden="true"
      >
        <template v-if="icon"><slot />{{ text }}</template>
        <template v-else>{{ text }}</template>
      </span>
    </Transition>
  </Teleport>
</template>

<style scoped>
.lw-tip {
  position: fixed;
  z-index: var(--lw-z-tooltip);
  width: max-content;
  max-width: 280px;
  padding: 6px 10px;
  border-radius: var(--lw-r-sm);
  border: 1px solid var(--lw-line-subtle);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
  color: var(--lw-ink-2);
  font-size: var(--lw-fs-11);
  line-height: 1.5;
  text-align: left;
  pointer-events: none;
}
/* 入场瞬时淡入；无退场过渡 = 移出立即消失（§二⑮） */
.lw-tip-enter-active {
  transition: opacity var(--dur-instant) var(--ease-standard);
}
.lw-tip-enter-from {
  opacity: 0;
}
</style>
