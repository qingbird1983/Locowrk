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
  /** 面板内主名下方的淡色说明（触发器只显主名；§3.32·七 v3.7）。 */
  desc?: string;
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
/** 打开时不预置高亮（用户裁定）：hover / 键盘移动才亮，-1 = 无活动项。 */
const activeIndex = ref(-1);
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
  // 带说明行的面板给足最小宽（说明不折成碎行）；纯主名面板随触发器
  const hasDesc = flat.value.some((o) => o.desc);
  const minW = hasDesc ? Math.max(Math.round(r.width), 224) : Math.round(r.width);
  // 面板右缘对齐触发器右缘（右对齐停靠，用户裁定），左缘回钳视口内
  const left = Math.max(8, Math.min(Math.round(r.right) - minW, window.innerWidth - minW - 8));
  popStyle.value = {
    left: `${left}px`,
    top: `${Math.round(r.bottom + 4)}px`,
    minWidth: `${minW}px`,
    maxWidth: "320px",
  };
  activeIndex.value = -1;
  activeIndex.value = Math.max(
    0,
    flat.value.findIndex((o) => o.value === props.modelValue),
  );
  open.value = true;
  void nextTick(() => {
    // 渲染后量实际尺寸再校正（首帧按触发器宽预估先落位防跳；但内容可把
    // 面板撑宽到 maxWidth——记录员的模型清单就比触发器宽，按预估宽右对
    // 齐会向右越界被窗口裁切，实抓于设置·会话记录员下拉）：横向 = 真右
    // 对齐 + 双向视口钳制；纵向 = 底部放不下向上翻。
    const pop = popRef.value;
    if (pop) {
      const w = pop.offsetWidth;
      const h = pop.offsetHeight;
      const left = Math.max(8, Math.min(Math.round(r.right) - w, window.innerWidth - w - 8));
      let top = Math.round(r.bottom + 4);
      if (top + h > window.innerHeight && r.top - h - 4 >= 0) {
        top = Math.round(r.top - h - 4);
      }
      popStyle.value = { ...popStyle.value, left: `${left}px`, top: `${top}px` };
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
  // 首次键盘移动落在当前选中项，再按才逐项走
  if (activeIndex.value === -1) {
    activeIndex.value = Math.max(
      0,
      flat.value.findIndex((o) => o.value === props.modelValue),
    );
    return;
  }
  activeIndex.value = (activeIndex.value + delta + n) % n;
}

function choose(i: number): void {
  if (i < 0) return;
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
    if (activeIndex.value >= 0) choose(activeIndex.value);
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
    <!-- 弹出面板动效（标准 §二①：滑动弹出 + 缓冲收尾，禁止硬切） -->
    <Transition name="lw-pop-down">
      <div
        v-if="open"
        ref="popRef"
        class="fixed z-[var(--lw-z-pop)] rounded-[var(--lw-r-md-plus)] border border-line bg-raised py-1 shadow-[var(--lw-shadow-pop)]"
        :style="popStyle"
        role="listbox"
        :aria-label="label"
        :aria-activedescendant="activeIndex >= 0 ? optionId(activeIndex) : undefined"
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
          class="px-3 pb-1 pt-1.5 text-[length:var(--lw-fs-11)] text-ink-4"
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
          <span class="min-w-0 flex-1">
            <span class="block truncate">{{ o.label }}</span>
            <span
              v-if="o.desc"
              class="mt-0.5 block text-[length:var(--lw-fs-11)] leading-snug text-ink-3"
              >{{ o.desc }}</span
            >
          </span>
          <Icon
            v-if="o.value === modelValue"
            name="check"
            :size="12"
            class="shrink-0 text-ink-3"
          />
        </button>
      </template>
      </div>
    </Transition>
  </Teleport>
</template>
