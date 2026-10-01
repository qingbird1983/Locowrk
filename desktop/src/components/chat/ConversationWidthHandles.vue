<script setup lang="ts">
/**
 * 主对话区列宽双柄（UI shell doc §2.7）：偏好值只存一份（settings
 * `chat.contentMaxWidth`），本组件把它 clamp 成实战宽度后写到对话区根的
 * `--lw-content-max`——转录列 / composer / 每条消息正文宽三处同吃这一个
 * 变量（同源是变量保证的，不是三处各自对齐）。双柄中心对称：两柄改的是
 * 同一个偏好值，指针位移 1px ⇒ 列宽变化 2px。触发区 12px 隐形热区（容器
 * pointer-events:none，下沿停在 composer 顶部）；可见提示 2×40 圆角竖条
 * hover/focus 淡入，拖拽中 40→56px 加深；拖拽零延迟（关过渡），仅程序化
 * 改宽（双击复位/键盘）走滑动过渡；pointercancel 与 Esc 回起始宽度且不
 * 落盘；双击复位缺省；键盘 ←/→ 步进 16px（Shift 32）方向按 data-side，
 * Home 复位 / End 最宽。实时上限随主区宽 ResizeObserver 收口。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import {
  CHAT_CONTENT_WIDTH_DEFAULT,
  CHAT_CONTENT_WIDTH_GUTTER,
  CHAT_CONTENT_WIDTH_MIN,
} from "../../lib/chat-width";

const props = defineProps<{
  /** composer 坞总高（热区下沿停在它顶部，不许盖住输入框）。 */
  bottom: number;
}>();

const { t } = useI18n();
const settings = useSettingsStore();

const rootEl = ref<HTMLElement | null>(null);
const areaWidth = ref(0);
let ro: ResizeObserver | null = null;
let raf = 0;
let pendingX = 0;
let animTimer: number | null = null;

const draggingSide = ref<"left" | "right" | null>(null);
const dragWidth = ref<number | null>(null);
let dragStartX = 0;
let dragStartWidth = 0;

const pref = computed(
  () => settings.view?.chat.contentMaxWidth ?? CHAT_CONTENT_WIDTH_DEFAULT,
);
/** 实战宽度 = clamp(偏好值, 560, 主区宽 − 48)；拖拽中用 live 值。 */
const liveWidth = computed(() => {
  const max = maxWidth.value;
  const w = dragWidth.value ?? pref.value;
  return Math.round(Math.min(max, Math.max(CHAT_CONTENT_WIDTH_MIN, w)));
});
const maxWidth = computed(() =>
  Math.max(CHAT_CONTENT_WIDTH_MIN, areaWidth.value - CHAT_CONTENT_WIDTH_GUTTER * 2),
);
/** 柄位置 = max(0, (主区宽 − 列宽) / 2 − 6px)，左右中心对称。 */
const handlePos = computed(() =>
  Math.max(0, (areaWidth.value - liveWidth.value) / 2 - 6),
);

function chatRoot(): HTMLElement | null {
  return rootEl.value?.closest("[data-chat-root]") as HTMLElement | null;
}
function applyVar() {
  chatRoot()?.style.setProperty("--lw-content-max", `${liveWidth.value}px`);
}
watch(liveWidth, applyVar);
// 拖拽中：全局锁光标禁选（§2.7 与 §2.3 侧栏拖柄同一套语义）
watch(draggingSide, (s) => {
  chatRoot()?.setAttribute("data-conv-resizing", s ? "true" : "false");
});

onMounted(() => {
  areaWidth.value = rootEl.value?.clientWidth ?? 0;
  ro = new ResizeObserver((entries) => {
    areaWidth.value = entries[0]?.contentRect.width ?? areaWidth.value;
  });
  if (rootEl.value) ro.observe(rootEl.value);
  applyVar();
});
onBeforeUnmount(() => {
  ro?.disconnect();
  if (raf) cancelAnimationFrame(raf);
  if (animTimer !== null) window.clearTimeout(animTimer);
});

// ---- 指针拖拽（rAF 每帧最多一次渲染；拖拽期间过渡关闭）。监听挂 window：
// setPointerCapture 在部分合成输入下拿不到有效 pointerId，元素级监听会
// 随指针离开 12px 热区即死——window 级两种情况都覆盖。 ----
function onPointerDown(e: PointerEvent, side: "left" | "right") {
  if (draggingSide.value) return;
  e.preventDefault();
  dragStartX = e.clientX;
  dragStartWidth = liveWidth.value;
  dragWidth.value = liveWidth.value;
  draggingSide.value = side;
  try {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  } catch {
    /* 无有效 pointerId：window 监听兜底 */
  }
  window.addEventListener("pointermove", onWinPointerMove);
  window.addEventListener("pointerup", onWinPointerUp);
  window.addEventListener("pointercancel", onWinPointerUp);
}
function onWinPointerMove(e: PointerEvent) {
  if (!draggingSide.value) return;
  pendingX = e.clientX;
  if (raf) return;
  raf = requestAnimationFrame(() => {
    raf = 0;
    const delta = pendingX - dragStartX;
    // 动一边等于两边对称各动一半：1px 指针位移 ⇒ 2px 列宽
    const d = draggingSide.value === "left" ? -delta : delta;
    dragWidth.value = dragStartWidth + d * 2;
  });
}
function onWinPointerUp() {
  if (!draggingSide.value) return;
  const w = liveWidth.value;
  endDrag();
  void settings.saveChat({ contentMaxWidth: w });
}
function cancelDrag() {
  // 取消就是取消：回起始宽度且不落盘
  endDrag();
}
function endDrag() {
  draggingSide.value = null;
  dragWidth.value = null;
  window.removeEventListener("pointermove", onWinPointerMove);
  window.removeEventListener("pointerup", onWinPointerUp);
  window.removeEventListener("pointercancel", onWinPointerUp);
  if (raf) {
    cancelAnimationFrame(raf);
    raf = 0;
  }
}

// ---- 程序化改宽（键盘 / 双击）：走 §6.6 ④ 滑动过渡，随即落盘 ----
function animateOnce() {
  const root = chatRoot();
  if (!root) return;
  root.classList.add("conv-anim");
  if (animTimer !== null) window.clearTimeout(animTimer);
  animTimer = window.setTimeout(() => root.classList.remove("conv-anim"), 300);
}
function saveWidth(w: number) {
  const max = maxWidth.value;
  void settings.saveChat({
    contentMaxWidth: Math.round(Math.min(max, Math.max(CHAT_CONTENT_WIDTH_MIN, w))),
  });
}
function onKeydown(e: KeyboardEvent, side: "left" | "right") {
  if (e.key === "Escape") {
    if (draggingSide.value) {
      e.preventDefault();
      cancelDrag();
    }
    return;
  }
  const step = e.shiftKey ? 32 : 16;
  const widen = side === "left" ? e.key === "ArrowLeft" : e.key === "ArrowRight";
  const narrow = side === "left" ? e.key === "ArrowRight" : e.key === "ArrowLeft";
  if (widen || narrow) {
    e.preventDefault();
    animateOnce();
    saveWidth(liveWidth.value + (widen ? step : -step));
  } else if (e.key === "Home") {
    e.preventDefault();
    animateOnce();
    saveWidth(CHAT_CONTENT_WIDTH_DEFAULT);
  } else if (e.key === "End") {
    e.preventDefault();
    animateOnce();
    saveWidth(maxWidth.value);
  }
}
function resetWidth() {
  animateOnce();
  saveWidth(CHAT_CONTENT_WIDTH_DEFAULT);
}
</script>

<template>
  <div
    ref="rootEl"
    class="conv-handles"
    :style="{ bottom: `${bottom}px` }"
  >
    <div
      v-for="side in (['left', 'right'] as const)"
      :key="side"
      class="conv-handle"
      :class="{ 'is-dragging': draggingSide === side }"
      :style="side === 'left' ? { left: `${handlePos}px` } : { right: `${handlePos}px` }"
      role="separator"
      aria-orientation="vertical"
      :aria-valuemin="CHAT_CONTENT_WIDTH_MIN"
      :aria-valuenow="liveWidth"
      :aria-valuemax="maxWidth"
      :aria-valuetext="t('chat.widthAria', { n: liveWidth })"
      tabindex="0"
      :data-side="side"
      @pointerdown="onPointerDown($event, side)"
      @dblclick.prevent="resetWidth"
      @keydown="onKeydown($event, side)"
    >
      <span class="conv-handle__bar" />
    </div>
  </div>
</template>

<style scoped>
.conv-handles {
  position: absolute;
  inset-inline: 0;
  top: 0;
  z-index: 11;
  pointer-events: none;
}
.conv-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 12px;
  pointer-events: auto;
  cursor: col-resize;
  touch-action: none;
}
.conv-handle:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px color-mix(in oklab, var(--lw-accent) 22%, transparent);
}
.conv-handle__bar {
  position: absolute;
  left: 5px;
  top: 50%;
  transform: translateY(-50%);
  width: 2px;
  height: 40px;
  border-radius: var(--lw-r-full);
  background: var(--lw-conv-handle);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard),
    height var(--dur-fast) var(--ease-standard);
}
.conv-handle:hover .conv-handle__bar,
.conv-handle:focus-visible .conv-handle__bar {
  opacity: 1;
}
/* 拖拽中：变长变深 = 正在拖；零延迟（§2.7 列宽必须跟手） */
.conv-handle.is-dragging .conv-handle__bar {
  opacity: 1;
  height: 56px;
  background: var(--lw-conv-handle-active);
  transition: none;
}
</style>
