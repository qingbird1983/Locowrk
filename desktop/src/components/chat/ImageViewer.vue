<script setup lang="ts">
/**
 * 图片查看器（批 E，多模态稿 §9.3 大图查看器转正）：全屏浮层 = 左右切换
 * （整条转录的图片序列，按钮 + ←/→）/ 缩放（＋/−/滚轮/双击复位）/ 拖移
 * / 另存为（原生对话框）/ 在文件夹中显示 / Esc 关闭。列表真源 = viewer
 * store（chat.viewerImages 顺序），本组件只渲染与转发。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { assetReveal, assetSaveAs } from "../../lib/api";
import { useToastStore } from "../../stores/toast";
import { useViewerStore } from "../../stores/viewer";

const viewer = useViewerStore();
const toast = useToastStore();
const { t } = useI18n();

const counter = computed(() =>
  viewer.list.length ? `${viewer.index + 1} / ${viewer.list.length}` : "",
);
const zoomPct = computed(() => `${Math.round(viewer.zoom * 100)}%`);
const transform = computed(
  () => `translate(${viewer.x}px, ${viewer.y}px) scale(${viewer.zoom})`,
);

/** 拖移态（指针捕获；缩放 =1 时拖移无意义但不拦——复位键一击归零）。 */
const dragging = ref(false);
let dragX = 0;
let dragY = 0;

function onPointerDown(e: PointerEvent) {
  dragging.value = true;
  dragX = e.clientX;
  dragY = e.clientY;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}
function onPointerMove(e: PointerEvent) {
  if (!dragging.value) return;
  viewer.panBy(e.clientX - dragX, e.clientY - dragY);
  dragX = e.clientX;
  dragY = e.clientY;
}
function onPointerUp() {
  dragging.value = false;
}

function onWheel(e: WheelEvent) {
  e.preventDefault();
  viewer.zoomBy(e.deltaY < 0 ? 1.2 : 1 / 1.2);
}

function onDblClick() {
  viewer.zoomSet(viewer.zoom === 1 ? 2 : 1);
}

async function onSave() {
  const cur = viewer.current;
  if (!cur) return;
  const dest = await assetSaveAs(cur.id, cur.mime, cur.name);
  if (dest) toast.push("ok", t("mm.savedAs", { path: dest }));
}

function onReveal() {
  const cur = viewer.current;
  if (!cur) return;
  void assetReveal(cur.id, cur.mime);
}

function onKey(e: KeyboardEvent) {
  if (!viewer.open) return;
  if (e.key === "Escape") viewer.close();
  else if (e.key === "ArrowLeft") viewer.nav(-1);
  else if (e.key === "ArrowRight") viewer.nav(1);
  else if (e.key === "+" || e.key === "=") viewer.zoomBy(1.2);
  else if (e.key === "-") viewer.zoomBy(1 / 1.2);
  else if (e.key === "0") viewer.zoomSet(1);
}

onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
watch(
  () => viewer.open,
  (open) => {
    document.body.classList.toggle("lw-viewer-locked", open);
  },
);
onBeforeUnmount(() => document.body.classList.remove("lw-viewer-locked"));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="viewer.open"
      class="viewer"
      role="dialog"
      :aria-label="t('mm.viewerTitle')"
      @wheel="onWheel"
    >
      <div class="viewer__backdrop" @click="viewer.close()" />
      <!-- 顶栏：名 · 序号 · 缩放组 · 动作 · 关闭（实底 elevated，浮层纪律） -->
      <div class="viewer__bar">
        <span class="viewer__name" :title="viewer.current?.name">{{
          viewer.current?.name
        }}</span>
        <span class="viewer__count">{{ counter }}</span>
        <span class="viewer__spacer" />
        <button
          class="viewer__ctl"
          type="button"
          :aria-label="t('mm.viewerZoomOut')"
          @click="viewer.zoomBy(1 / 1.2)"
        >
          <Icon name="min" :size="14" />
        </button>
        <button
          class="viewer__ctl viewer__pct"
          type="button"
          :aria-label="t('mm.viewerZoomReset')"
          @click="viewer.zoomSet(1)"
        >
          {{ zoomPct }}
        </button>
        <button
          class="viewer__ctl"
          type="button"
          :aria-label="t('mm.viewerZoomIn')"
          @click="viewer.zoomBy(1.2)"
        >
          <Icon name="plus" :size="14" />
        </button>
        <span class="viewer__sep" />
        <button class="viewer__ctl" type="button" @click="onSave">
          <Icon name="download" :size="14" />
          {{ t("mm.saveAsset") }}
        </button>
        <button class="viewer__ctl" type="button" @click="onReveal">
          <Icon name="folderOpen" :size="14" />
          {{ t("mm.revealAsset") }}
        </button>
        <button
          class="viewer__ctl"
          type="button"
          :aria-label="t('mm.viewerClose')"
          @click="viewer.close()"
        >
          <Icon name="close" :size="14" />
        </button>
      </div>
      <!-- 舞台：图居中，transform 承载缩放/拖移；backdrop 点击关闭 -->
      <div
        class="viewer__stage"
        @pointerdown="onPointerDown"
        @pointermove="onPointerMove"
        @pointerup="onPointerUp"
        @pointercancel="onPointerUp"
        @dblclick="onDblClick"
      >
        <span v-if="viewer.loading" class="viewer__hint">{{ t("mm.uploading") }}</span>
        <span v-else-if="viewer.error" class="viewer__hint viewer__err">{{
          viewer.error
        }}</span>
        <img
          v-else-if="viewer.src"
          class="viewer__img"
          :class="{ 'is-dragging': dragging }"
          :src="viewer.src"
          :alt="viewer.current?.name"
          draggable="false"
          :style="{ transform }"
        />
      </div>
      <!-- 左右切换（越界隐藏） -->
      <button
        v-if="viewer.canPrev"
        class="viewer__nav viewer__nav--l"
        type="button"
        :aria-label="t('mm.viewerPrev')"
        @click.stop="viewer.nav(-1)"
      >
        <Icon name="chevronLeft" :size="18" />
      </button>
      <button
        v-if="viewer.canNext"
        class="viewer__nav viewer__nav--r"
        type="button"
        :aria-label="t('mm.viewerNext')"
        @click.stop="viewer.nav(1)"
      >
        <Icon name="chevronRight" :size="18" />
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.viewer {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: flex;
  flex-direction: column;
}
.viewer__backdrop {
  position: absolute;
  inset: 0;
  background: rgb(var(--lw-rgb-ink) / 72%);
}
.viewer__bar {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  margin: 10px 12px 0; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值10/12,§二⑬) */
  padding: 6px 10px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值6/10,§二⑬) */
  border-radius: var(--lw-r-lg);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
}
.viewer__name {
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-1);
}
.viewer__count {
  font-family: var(--font-mono);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
}
.viewer__spacer {
  flex: 1;
}
.viewer__sep {
  width: 1px;
  height: 16px;
  background: var(--lw-line-subtle);
}
.viewer__ctl {
  display: inline-flex;
  align-items: center;
  gap: 4px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值4,§二⑬) */
  padding: 4px 8px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值4/8,§二⑬) */
  border-radius: var(--lw-r-sm);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-2);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.viewer__ctl:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.viewer__pct {
  min-width: 44px;
  justify-content: center;
  font-family: var(--font-mono);
  font-size: var(--lw-fs-11);
}
.viewer__stage {
  position: relative;
  z-index: 1;
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  touch-action: none;
}
.viewer__img {
  max-width: 92%;
  max-height: 92%;
  border-radius: var(--lw-r-md);
  box-shadow: var(--lw-shadow-raised);
  user-select: none;
  transition: transform var(--dur-fast) var(--ease-out);
}
/* 拖移中关过渡（跟手），松手恢复 */
.viewer__img.is-dragging {
  transition: none;
}
.viewer__hint {
  font-size: var(--lw-fs-13);
  color: rgb(var(--lw-rgb-bg) / 88%);
}
.viewer__err {
  max-width: 60%;
  overflow-wrap: anywhere;
  text-align: center;
}
.viewer__nav {
  position: absolute;
  top: 50%;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: var(--lw-r-full);
  color: var(--lw-ink-1);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
  transform: translateY(-50%);
  transition: background var(--dur-fast) var(--ease-standard);
}
.viewer__nav:hover {
  background: var(--lw-wash-2);
}
.viewer__nav--l {
  left: 16px;
}
.viewer__nav--r {
  right: 16px;
}
</style>

<style>
/* 查看器打开期间锁背景滚动（body 级，组件卸载自清理） */
body.lw-viewer-locked {
  overflow: hidden;
}
</style>
