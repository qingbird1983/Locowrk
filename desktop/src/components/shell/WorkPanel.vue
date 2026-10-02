<script setup lang="ts">
/**
 * 工作面板（UI shell 文档 §2.5，v2.52）：工具产物的第二展示面。三栏预算
 * = 容器宽 − 侧栏宽（折叠算 0）− 主区地板 450（硬的：拖宽顶到预算先折叠
 * 侧栏再贴新预算钳制）；拖柄 = setPointerCapture + 全局锁 + ::after 圆角
 * 条 + 键盘 16px/Home/End + 双击复位 360 + aria separator（valuemax=预算
 * 动态）；头部 = 页签条（可关闭）+ `+`（回启动器）+ 最大化/还原 + 关闭；
 * 空态 = 启动器清单（只列已实装条目：§3.16 当前会话可看的子转录）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore, type ToolItem } from "../../stores/chat";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { MAIN_FLOOR, PANEL_MIN } from "../../lib/work-panel-preference";
import { clip } from "../../lib/format";
import { Icon } from "../../lib/icons";
import SubagentTranscriptTab from "./SubagentTranscriptTab.vue";

const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const wp = useWorkPanelStore();

// ---- 三栏预算（§2.5）----
const viewport = ref(window.innerWidth);
function onResize() {
  viewport.value = window.innerWidth;
}
onMounted(() => window.addEventListener("resize", onResize));
onBeforeUnmount(() => window.removeEventListener("resize", onResize));

const sidebarW = computed(() => (shell.sidebarCollapsed ? 0 : shell.sidebarWidth));
const budget = computed(() => Math.max(1, viewport.value - sidebarW.value - MAIN_FLOOR));
const width = computed(() => wp.clampTo(budget.value));

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

// ---- 启动器清单（空态，§2.5/§3.14）：当前会话可看的子转录 ----
const launcherItems = computed(() => {
  const out: { childId: string; title: string }[] = [];
  const seen = new Set<string>();
  for (const item of chat.items) {
    if (item.kind !== "tool" || item.pending || !item.ok) continue;
    const tool = item as ToolItem;
    if (tool.tool !== "subagent_run") continue;
    const childId = (tool.output as { child_session_id?: string } | null)?.child_session_id;
    if (!childId || seen.has(childId)) continue;
    seen.add(childId);
    out.push({
      childId,
      title: clip(String((tool.args as { prompt?: unknown } | null)?.prompt ?? ""), 40),
    });
  }
  return out;
});

function openLauncherItem(childId: string, title: string) {
  wp.openSubTab(childId, title);
}
</script>

<template>
  <aside
    class="work-panel relative flex h-full shrink-0 flex-col overflow-hidden bg-side"
    :style="{ width: `${width}px` }"
    :aria-label="t('panel.title')"
  >
    <!-- 头部：页签条（可关闭）+ 动作（`+` 回启动器 / 最大化 / 关闭面板） -->
    <header
      class="flex h-[var(--lw-toolbar-h)] shrink-0 items-center gap-1 border-b border-line-subtle px-2"
    >
      <div class="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        <button
          v-for="tab in wp.tabs"
          :key="tab.id"
          type="button"
          class="panel-tab"
          :data-active="wp.activeTabId === tab.id || undefined"
          :title="tab.title"
          @click="wp.activeTabId = tab.id"
        >
          <Icon name="bot" :size="12" aria-hidden="true" />
          <span class="panel-tab__title">{{ tab.title }}</span>
          <span
            class="panel-tab__close"
            role="button"
            :aria-label="t('panel.closeTab')"
            @click.stop="wp.closeTab(tab.id)"
          >
            <Icon name="close" :size="10" aria-hidden="true" />
          </span>
        </button>
      </div>
      <button
        class="ctl"
        type="button"
        :title="t('panel.launcher')"
        :aria-label="t('panel.launcher')"
        @click="wp.activeTabId = null"
      >
        <Icon name="plus" aria-hidden="true" />
      </button>
      <button
        class="ctl"
        type="button"
        :title="wp.maximized ? t('panel.restore') : t('panel.maximize')"
        :aria-label="wp.maximized ? t('panel.restore') : t('panel.maximize')"
        @click="wp.toggleMaximized(budget)"
      >
        <Icon name="max" :size="13" aria-hidden="true" />
      </button>
      <button
        class="ctl"
        type="button"
        :title="t('panel.closePanel')"
        :aria-label="t('panel.closePanel')"
        @click="wp.setOpen(false)"
      >
        <Icon name="close" :size="13" aria-hidden="true" />
      </button>
    </header>

    <!-- 内容：活动页签 / 启动器清单（§3.14：不是空态文案） -->
    <div class="flex min-h-0 flex-1 flex-col">
      <SubagentTranscriptTab
        v-if="wp.activeTabId"
        :key="wp.activeTabId"
        :child-id="wp.activeTabId"
      />
      <div v-else class="scroll-y flex-1 px-2 py-2">
        <div class="side-label px-1">{{ t("panel.launcher") }}</div>
        <button
          v-for="item in launcherItems"
          :key="item.childId"
          type="button"
          class="launcher-row"
          @click="openLauncherItem(item.childId, item.title)"
        >
          <Icon name="bot" :size="14" aria-hidden="true" />
          <span class="launcher-row__title">{{ item.title }}</span>
        </button>
        <p class="px-2 py-3 text-[12px] leading-relaxed text-ink-4">
          {{ t("panel.launcherHint") }}
        </p>
      </div>
    </div>

    <!-- 拖柄（§2.5）：嵌面板左缘，10px 热区 + 2×32 圆角条提示 -->
    <button
      class="panel-grip"
      type="button"
      role="separator"
      aria-orientation="horizontal"
      :aria-label="t('panel.resize')"
      :aria-valuemin="PANEL_MIN"
      :aria-valuemax="budget"
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
.work-panel {
  border-left: 1px solid var(--lw-line-subtle);
}
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

/* 页签（§2.5）：28px 圆角 8、选中 wash-3（高亮瓦片不粘连，§7 规则） */
.panel-tab {
  display: inline-flex;
  height: 28px;
  max-width: 180px;
  flex: 0 0 auto;
  align-items: center;
  gap: 5px;
  border-radius: var(--lw-r-md);
  padding: 0 6px 0 8px;
  color: var(--lw-ink-2);
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
  font-size: 12px;
}
.panel-tab__close {
  display: inline-flex;
  width: 14px;
  height: 14px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-xs);
  color: var(--lw-ink-4);
}
.panel-tab__close:hover {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}

/* 启动器行（§3.14 列表行卡）：图标 + 一句可做的事 */
.launcher-row {
  display: flex;
  width: 100%;
  min-height: 28px;
  align-items: center;
  gap: 8px;
  border-radius: var(--lw-r-md);
  padding: 0 8px;
  color: var(--lw-ink-2);
  text-align: left;
}
.launcher-row:hover {
  background: var(--lw-wash-2);
}
.launcher-row__title {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
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
.side-label {
  padding: 6px 8px 4px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
</style>
