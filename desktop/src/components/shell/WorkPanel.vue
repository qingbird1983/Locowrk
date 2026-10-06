<script setup lang="ts">
/**
 * 工作面板（UI shell 文档 §2.5，v2.52）：工具产物的第二展示面。三栏预算
 * = 容器宽 − 侧栏宽（折叠算 0）− 主区地板 450（硬的：拖宽顶到预算先折叠
 * 侧栏再贴新预算钳制）；拖柄 = setPointerCapture + 全局锁 + ::after 圆角
 * 条 + 键盘 16px/Home/End + 双击复位 360 + aria separator（valuemax=预算
 * 动态）；头部 = 页签条（可关闭）+ `+`（回启动器）+ 最大化/还原（v2.60：
 * 斜对角箭头图标；关闭面板钮退役——开合统一走顶栏开关，v2.60 修复批）；
 * 空态 = 启动器清单（只列已实装条目：§3.16 当前会话可看的子转录）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { MAIN_FLOOR, PANEL_MIN } from "../../lib/work-panel-preference";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";
import SubagentTranscriptTab from "./SubagentTranscriptTab.vue";
import ReviewTab from "./ReviewTab.vue";
import WorkPanelLauncher from "./WorkPanelLauncher.vue";

const { t } = useI18n();
const shell = useShellStore();
const wp = useWorkPanelStore();

// ---- 三栏预算（§2.5）----
const viewport = ref(window.innerWidth);
function onResize() {
  viewport.value = window.innerWidth;
}
onMounted(() => window.addEventListener("resize", onResize));
onBeforeUnmount(() => window.removeEventListener("resize", onResize));

const activeTab = computed(() =>
  wp.tabs.find((t) => t.id === wp.activeTabId) ?? null,
);

const sidebarW = computed(() => (shell.sidebarCollapsed ? 0 : shell.sidebarWidth));
/** 常态预算：主区保地板 450（§2.5）。 */
const budget = computed(() => Math.max(1, viewport.value - sidebarW.value - MAIN_FLOOR));
/** 最大化预算（v2.60 修复批）：顶到侧栏左缘直接覆盖对话区——侧栏折叠 =
 * 满窗口，侧栏展开 = 视口 − 侧栏宽（主区地板在最大化态不保）。 */
const fullBudget = computed(() => Math.max(1, viewport.value - sidebarW.value));
const width = computed(() =>
  wp.maximized ? fullBudget.value : wp.clampTo(budget.value),
);

// ---- 拖柄（§2.5）----
const dragging = ref(false);
let startX = 0;
let startW = 0;

function onPointerDown(e: PointerEvent) {
  dragging.value = true;
  startX = e.clientX;
  startW = width.value;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  document.documentElement.dataset.workPanelResizing = "true";
}
function onPointerMove(e: PointerEvent) {
  if (!dragging.value) return;
  // 面板贴右缘：向左拖 = 变宽；顶到预算且侧栏还开着 → 先折叠侧栏（地板硬）
  const desired = startW + (startX - e.clientX);
  if (desired > budget.value && !shell.sidebarCollapsed) {
    shell.toggleSidebar();
  }
  const b = viewport.value - (shell.sidebarCollapsed ? 0 : shell.sidebarWidth) - MAIN_FLOOR;
  wp.setWidth(Math.max(PANEL_MIN, Math.min(desired, b)));
}
function endDrag(release: boolean) {
  if (release) {
    delete document.documentElement.dataset.workPanelResizing;
  }
  dragging.value = false;
}
function onKeyDown(e: KeyboardEvent) {
  const step = 16;
  const widen = () => wp.setWidth(Math.min(budget.value, wp.clampTo(budget.value) + step));
  const narrow = () => wp.setWidth(Math.max(PANEL_MIN, wp.clampTo(budget.value) - step));
  if (e.key === "ArrowLeft") widen();
  else if (e.key === "ArrowRight") narrow();
  else if (e.key === "Home") wp.setWidth(budget.value);
  else if (e.key === "End") wp.setWidth(Math.min(PANEL_MIN, budget.value));
  else return;
  e.preventDefault();
}

// ---- 启动器条目打开（数据与行渲染在 WorkPanelLauncher）----
function openSub(childId: string, title: string) {
  wp.openSubTab(childId, title);
}
function openReview(item: { callId: string; path: string; diff: string }) {
  wp.openReviewTab(item.callId, item.path, item.diff);
}
</script>

<template>
  <aside
    class="work-panel relative z-20 flex h-full shrink-0 flex-col overflow-hidden bg-dock"
    :style="{ width: `${width}px` }"
    :aria-label="t('panel.title')"
  >
    <!-- 头部：页签条（可关闭）+ 动作（`+` 回启动器 / 最大化 / 关闭面板）。
         面板恒在窗口最右，46px 带右端的窗口控制车道由它接管（§2.2-5）：
         padding-right = 车道 + 40px 顶栏开关钮位，顶栏同刻让位 -->
    <header
      class="flex h-[var(--lw-toolbar-h)] shrink-0 items-center gap-1 px-3"
      style="padding-right: calc(var(--lw-wincontrols) + 40px)"
    >
      <div class="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        <HoverTip
          v-for="tab in wp.tabs"
          :key="tab.id"
          class="shrink-0"
          :text="tab.title"
          :label="tab.title"
        >
          <button
            type="button"
            class="panel-tab"
            :data-active="wp.activeTabId === tab.id || undefined"
            @click="wp.activeTabId = tab.id"
          >
            <Icon
              v-if="tab.kind === 'subagent'"
              name="bot"
              :size="12"
              aria-hidden="true"
            />
            <span class="panel-tab__title">{{ tab.title }}</span>
            <span
              class="panel-tab__close"
              role="button"
              :aria-label="t('panel.closeTab')"
              @click.stop="wp.closeTab(tab.id)"
            >
              <Icon name="close" :size="12" aria-hidden="true" />
            </span>
          </button>
        </HoverTip>
      </div>
      <HoverTip :text="t('panel.launcher')">
        <button
          class="ctl"
          type="button"
          :aria-label="t('panel.launcher')"
          @click="wp.activeTabId = null"
        >
          <Icon name="plus" aria-hidden="true" />
        </button>
      </HoverTip>
      <HoverTip
        :text="wp.maximized ? t('panel.restore') : t('panel.maximize')"
      >
        <button
          class="ctl"
          type="button"
          :aria-label="wp.maximized ? t('panel.restore') : t('panel.maximize')"
          @click="wp.toggleMaximized(fullBudget)"
        >
          <Icon name="maxDiag" :size="14" aria-hidden="true" />
        </button>
      </HoverTip>
    </header>

    <!-- 内容：活动页签（按类型分派）/ 启动器清单（§3.14：不是空态文案；
         数据与行渲染在 WorkPanelLauncher，§8.4 规划的拆分件） -->
    <div class="flex min-h-0 flex-1 flex-col">
      <template v-if="activeTab">
        <SubagentTranscriptTab
          v-if="activeTab.kind === 'subagent'"
          :key="activeTab.id"
          :child-id="activeTab.id"
        />
        <ReviewTab
          v-else-if="activeTab.kind === 'review' && activeTab.path && activeTab.diff"
          :key="activeTab.id"
          :path="activeTab.path"
          :diff="activeTab.diff"
        />
      </template>
      <WorkPanelLauncher
        v-else
        @open-sub="openSub"
        @open-review="openReview"
      />
    </div>

    <!-- 拖柄（§2.5）：嵌面板左缘，10px 热区 + 2×32 圆角条提示 -->
    <button
      class="panel-grip"
      type="button"
      role="separator"
      aria-orientation="vertical"
      :aria-label="t('panel.resize')"
      :aria-valuemin="PANEL_MIN"
      :aria-valuemax="wp.maximized ? fullBudget : budget"
      :aria-valuenow="width"
      :aria-valuetext="t('panel.resizeText', { n: width })"
      :data-dragging="dragging ? '' : undefined"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="endDrag(true)"
      @pointercancel="endDrag(true)"
      @lostpointercapture="endDrag(true)"
      @keydown="onKeyDown"
      @dblclick="wp.resetWidth()"
    />
  </aside>
</template>

<style scoped>
/* 面板列底 = bg-dock（§6.1 表专token）；平面内无边框——与主区的层次
 * 靠底色差表达（§6.1 平面内分层铁则，v3.8 走查修正）。z-20 = 压过对话区
 * 的浮起 composer 坞（z-12）/列宽柄（z-11）——v2.60 最大化直接覆盖对话区
 * 时浮层残影不再透出（常态开面板无水平重叠，此 z 只在覆盖态生效） */
.ctl {
  display: inline-flex;
  width: var(--lw-ctl);
  height: var(--lw-ctl);
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-md);
  color: var(--lw-ink-2);
}
.ctl:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}

/* 页签（原型 .tab）：28px 高、圆角 10、常态 wash-1、选中 wash-3、
 * max-width 200、字 12.5px；仅子 Agent 页签挂 bot 字形 */
.panel-tab {
  display: inline-flex;
  height: 28px;
  max-width: 200px;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--lw-gap-6);
  border-radius: var(--lw-r-sm);
  padding: 0 var(--lw-gap-4) 0 var(--lw-gap-10);
  font-size: var(--lw-fs-12half);
  line-height: 1.3;
  color: var(--lw-ink-2);
  background: var(--lw-wash-1);
}
.panel-tab:hover {
  background: var(--lw-wash-2);
}
.panel-tab[data-active] {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}
.panel-tab__title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lw-fs-12half);
}
.panel-tab__close {
  display: inline-flex;
  width: 20px;
  height: 20px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-2xs);
  color: var(--lw-ink-4);
}
.panel-tab__close:hover {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}

/* 拖柄（§2.5）：inset-block 0 / left -5 / width 10 / z-30；提示 2×32
 * 圆角条 hover/active/聚焦/拖拽显形，聚焦与拖拽换 accent，聚焦不画外框 */
.panel-grip {
  position: absolute;
  top: 0;
  bottom: 0;
  left: -5px;
  width: 10px;
  z-index: 30;
  cursor: col-resize;
  touch-action: none;
  user-select: none;
}
.panel-grip::after {
  content: "";
  position: absolute;
  top: 50%;
  left: 4px;
  width: 2px;
  height: 32px;
  border-radius: var(--lw-r-full);
  background: color-mix(in oklab, var(--lw-ink-1) 20%, transparent);
  opacity: 0;
  transform: translateY(-50%);
  transition: opacity var(--dur-fast) var(--ease-out);
}
.panel-grip:hover::after,
.panel-grip:active::after,
.panel-grip:focus-visible::after,
.panel-grip[data-dragging]::after {
  opacity: 1;
}
.panel-grip:active::after,
.panel-grip:focus-visible::after,
.panel-grip[data-dragging]::after {
  background: var(--lw-accent);
}
.panel-grip:focus-visible {
  outline: none;
}
</style>
