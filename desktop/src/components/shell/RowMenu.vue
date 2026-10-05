<script setup lang="ts">
/**
 * 行尾 `…` / 右键菜单（UI shell 文档 §3.20）：min-width 184、宽随内容、
 * padding 5px、圆角 14、bg-elevated + 1px line 围边 + 微弱悬浮影、z=浮层
 * 档；项 7px 8px、12.5px、圆角 10、hover wash-2；危险项整行 danger 字色 +
 * hover danger 16% 底、图标继承字色；先测量再显示（visibility:hidden 开
 * 局，不在视口原点闪帧）、超出视口翻转限位；Esc 关闭并把焦点还给触发控
 * 件（父组件在 close 后聚焦触发钮）；↑/↓ 移动焦点、Enter 执行。进退场
 * 动画由挂载点的 Teleport + Transition 接管（本组件根 = 单元素）。
 */
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";

export interface RowMenuItem {
  id: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
  /** 分隔线（占位项，label 不渲染）。 */
  separator?: boolean;
  /** 选择后保持菜单开启（两步确认等）；关闭由父组件收起 v-if。 */
  keepOpen?: boolean;
}

const props = defineProps<{ items: RowMenuItem[]; x: number; y: number }>();
const emit = defineEmits<{ select: [id: string]; close: [] }>();

const root = ref<HTMLElement | null>(null);
const pos = ref({ left: -9999, top: -9999 });
const active = ref(-1);

function enabledIndices(): number[] {
  return props.items
    .map((it, i) => (it.separator || it.disabled ? -1 : i))
    .filter((i) => i >= 0);
}

async function place() {
  const el = root.value;
  if (!el) return;
  const r = el.getBoundingClientRect();
  let left = props.x;
  let top = props.y;
  if (left + r.width > window.innerWidth - 8) {
    left = Math.max(8, props.x - r.width);
  }
  if (top + r.height > window.innerHeight - 8) {
    top = Math.max(8, props.y - r.height);
  }
  pos.value = { left, top };
  el.style.visibility = "visible";
  const first = enabledIndices()[0];
  active.value = first ?? -1;
  focusActive();
}

function focusActive() {
  const buttons = root.value?.querySelectorAll<HTMLElement>("[role='menuitem']");
  const el = buttons?.[active.value];
  el?.focus();
}

function move(delta: 1 | -1) {
  const ids = enabledIndices();
  if (ids.length === 0) return;
  const at = ids.indexOf(active.value);
  const next = at === -1 ? 0 : (at + delta + ids.length) % ids.length;
  active.value = ids[next];
  focusActive();
}

function pick(item: RowMenuItem) {
  if (item.separator || item.disabled) return;
  emit("select", item.id);
  // 关闭权在父组件（v-if 收起）——keepOpen 项（两步确认）父组件自会决定
  if (!item.keepOpen) emit("close");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.stopPropagation();
    emit("close");
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    move(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    move(-1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    const item = props.items[active.value];
    if (item) pick(item);
  }
}

function onOutside(e: PointerEvent) {
  if (root.value && !root.value.contains(e.target as Node)) emit("close");
}

onMounted(async () => {
  document.addEventListener("pointerdown", onOutside, true);
  await nextTick();
  place();
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onOutside, true);
});
</script>

<template>
  <!-- 不自带 Teleport：挂载点用 <Teleport><Transition name="lw-pop-down">
       包裹（组件根保持单元素，Transition 才接得住进退场——退场动画由此
       生效，标准 §二① 禁止无退场） -->
  <div
    ref="root"
    class="row-menu"
    role="menu"
    :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
    @keydown="onKey"
  >
      <template v-for="item in items" :key="item.id">
        <div v-if="item.separator" class="row-menu__sep" role="separator" />
        <button
          v-else
          type="button"
          role="menuitem"
          class="row-menu__item"
          :class="{
            'row-menu__item--danger': item.danger,
            'row-menu__item--disabled': item.disabled,
          }"
          :disabled="item.disabled"
          @click="pick(item)"
        >
          {{ item.label }}
        </button>
      </template>
  </div>
</template>

<style scoped>
.row-menu {
  position: fixed;
  z-index: var(--lw-z-pop);
  visibility: hidden; /* 先测量再显示（§3.20）：避免在视口原点闪一帧 */
  min-width: 184px;
  width: max-content;
  padding: 5px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值5,§二⑬) */
  border-radius: var(--lw-r-md-plus); /* §3.20 圆角 14 */
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-pop);
}
.row-menu__item {
  display: block;
  width: 100%;
  padding: 7px var(--lw-gap-8); /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  border-radius: var(--lw-r-md);
  text-align: left;
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-1);
}
.row-menu__item:hover,
.row-menu__item:focus-visible {
  background: var(--lw-wash-2);
  outline: none;
}
.row-menu__item--danger {
  color: var(--lw-danger);
}
.row-menu__item--danger:hover,
.row-menu__item--danger:focus-visible {
  background: color-mix(in oklab, var(--lw-danger) 16%, transparent);
}
.row-menu__item--disabled {
  color: var(--lw-ink-5);
  cursor: not-allowed;
}
.row-menu__item--disabled:hover {
  background: transparent;
}
.row-menu__sep {
  height: 1px;
  margin: 5px var(--lw-gap-6); /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值5,§二⑬) */
  background: var(--lw-line-subtle);
}
</style>
