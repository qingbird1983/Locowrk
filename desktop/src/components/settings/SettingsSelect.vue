<script setup lang="ts">
/**
 * 自定义下拉（UI shell 文档 §3.32·七 v3.6 两色控件体系，用户裁定替代
 * 原生 select）：触发器 = 白底台面按钮（当前值 + chevron）；弹出面板 =
 * 白底（raised）+ 1px line + r-md-plus + shadow-raised，teleport 到 body
 * 定位。选项 hover wash-2、选中 wash-3（与导航轨/列表选中统一，不吃系统
 * 蓝高亮）；多类别分组时组间发丝线 + 11px 组名。键盘 ↑↓/Home/End/Enter/Esc
 * 全套；label 由调用方 t() 后传入。聚焦不出环（§7-12 v3.6 字段类豁免）。
 */
import { computed, nextTick, onBeforeUnmount, ref } from "vue";
import { Icon } from "../../lib/icons";

export interface SelectFlat {
  value: string;
  label: string;
}
export interface SelectGroup {
  label: string;
  options: SelectFlat[];
}
export type SelectEntry = SelectFlat | SelectGroup;

const props = defineProps<{
  modelValue: string;
  entries: SelectEntry[];
  /** 无障碍名（调用方 t() 后传入）。 */
  label: string;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const listId = `lw-select-${Math.random().toString(36).slice(2, 9)}`;

const open = ref(false);
const activeIndex = ref(0);
const triggerRef = ref<HTMLButtonElement | null>(null);
const popRef = ref<HTMLDivElement | null>(null);
const popStyle = ref<Record<string, string>>({});

/** 展平后的选项（选中回显与键盘索引用）。 */
const flat = computed<SelectFlat[]>(() =>
  props.entries.flatMap((e) => ("options" in e ? e.options : [e])),
);
const currentLabel = computed(
  () => flat.value.find((o) => o.value === props.modelValue)?.label ?? "",
);

function optionId(i: number): string {
  return `${listId}-opt-${i}`;
}

function openList(): void {
  if (open.value) return;
  const el = triggerRef.value;
  if (!el) return;
  const r = el.getBoundingClientRect();
  popStyle.value = {
    left: `${Math.round(r.left)}px`,
    top: `${Math.round(r.bottom + 4)}px`,
    minWidth: `${Math.round(r.width)}px`,
  };
  activeIndex.value = Math.max(
    0,
    flat.value.findIndex((o) => o.value === props.modelValue),
  );
  open.value = true;
  void nextTick(() => {
    // 底部放不下就向上翻（渲染后量实际高度）
    const pop = popRef.value;
    if (pop) {
      const h = pop.offsetHeight;
      if (r.bottom + 4 + h > window.innerHeight && r.top - h - 4 >= 0) {
        popStyle.value = { ...popStyle.value, top: `${Math.round(r.top - h - 4)}px` };
      }
      pop.focus();
    }
  });
  window.addEventListener("pointerdown", onOutside, true);
  window.addEventListener("scroll", close, true);
  window.addEventListener("resize", close);
}

function close(): void {
  if (!open.value) return;
  open.value = false;
  window.removeEventListener("pointerdown", onOutside, true);
  window.removeEventListener("scroll", close, true);
  window.removeEventListener("resize", close);
}

function toggle(): void {
  if (open.value) close();
  else openList();
}

/** 点在触发器上时不关（触发器自己 toggle）；点外关。 */
function onOutside(e: PointerEvent): void {
  const t = e.target as Node;
  if (popRef.value?.contains(t) || triggerRef.value?.contains(t)) return;
  close();
}

function move(delta: number): void {
  const n = flat.value.length;
  if (!n) return;
  activeIndex.value = (activeIndex.value + delta + n) % n;
}

function choose(i: number): void {
  const o = flat.value[i];
  if (!o) return;
  emit("update:modelValue", o.value);
  close();
  triggerRef.value?.focus();
}

/** 键盘（挂在弹层容器上；打开即聚焦它）。Esc 关并回落触发器。 */
function onPopKeydown(e: KeyboardEvent): void {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    move(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    move(-1);
  } else if (e.key === "Home") {
    e.preventDefault();
    activeIndex.value = 0;
  } else if (e.key === "End") {
    e.preventDefault();
    activeIndex.value = Math.max(0, flat.value.length - 1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    choose(activeIndex.value);
  } else if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    close();
    triggerRef.value?.focus();
  }
}

onBeforeUnmount(close);
</script>

<template>
  <button
    ref="triggerRef"
    type="button"
    class="lw-btn flex min-w-0 items-center gap-2 px-3 py-1.5 text-xs text-ink-1"
    :aria-label="label"
    aria-haspopup="listbox"
    :aria-expanded="open"
    @click="toggle"
    @keydown.down.prevent="openList"
  >
    <span class="min-w-0 flex-1 truncate text-left">{{ currentLabel }}</span>
    <Icon name="chevronDown" :size="12" class="shrink-0 text-ink-4" />
  </button>

  <Teleport to="body">
    <div
      v-if="open"
      ref="popRef"
      class="fixed z-[80] rounded-[var(--lw-r-md-plus)] border border-line bg-raised py-1 shadow-raised"
      :style="popStyle"
      role="listbox"
      :aria-label="label"
      :aria-activedescendant="optionId(activeIndex)"
      tabindex="-1"
      @keydown="onPopKeydown"
    >
      <template v-for="(entry, gi) in entries" :key="'options' in entry ? entry.label : entry.value">
        <!-- 组间发丝线 + 组名（仅分组条目渲染；首组无线） -->
        <div
          v-if="'options' in entry && gi > 0"
          class="mx-3 my-1 border-t border-line-subtle"
          role="separator"
        />
        <p
          v-if="'options' in entry"
          class="px-3 pb-1 pt-1.5 text-[11px] text-ink-4"
        >
          {{ entry.label }}
        </p>
        <button
          v-for="o in 'options' in entry ? entry.options : [entry]"
          :id="optionId(flat.indexOf(o))"
          :key="o.value"
          type="button"
          role="option"
          :aria-selected="o.value === modelValue"
          class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs"
          :class="
            o.value === modelValue
              ? 'bg-wash-3 font-medium text-ink-1'
              : flat.indexOf(o) === activeIndex
                ? 'bg-wash-2 text-ink-1'
                : 'text-ink-2'
          "
          @mouseenter="activeIndex = flat.indexOf(o)"
          @click="choose(flat.indexOf(o))"
        >
          <span class="min-w-0 flex-1 truncate">{{ o.label }}</span>
          <Icon
            v-if="o.value === modelValue"
            name="check"
            :size="12"
            class="shrink-0 text-ink-3"
          />
        </button>
      </template>
    </div>
  </Teleport>
</template>
