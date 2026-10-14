<script setup lang="ts">
/**
 * 画布（队列 Q14 第三刀 · §二⑯ 六件构件的装配处）。
 *
 * 分层照 §二⑯ 构件表：视口（不变换的裁剪框，`overflow:hidden` + `--lw-bg`，到顶、
 * 无标题带、无发丝线）→ 世界层（**唯一**被 `translate…scale…` 的件，尺寸由内容测量
 * 算）→ 网格底（世界层内，随世界平移缩放，纯装饰）→ 节点（四列，`BoardNode` 一只
 * 壳件）→ 连线（SVG 三次贝塞尔）→ 汇聚面板（第 4 列，`StreamPanel`）。缩放控件条、
 * 会话浮层、提示行是**台面件**：`--lw-bg-elevated` + `--lw-shadow-pop`，不写 z-index。
 *
 * 本件只管"壳与层"：几何在 `use-board-layout.ts`，三交互在
 * `use-board-interaction.ts`，尺寸与纪律常量在 `lib/board-model.ts`——同一个改动理由
 * 只落一个文件（§模块规模纪律 三问自检）。
 */
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import BoardNode from "./BoardNode.vue";
import StreamPanel from "./StreamPanel.vue";
import TreeBadge from "../TreeBadge.vue";
import { useBoardMetrics } from "./use-board-metrics";
import { useBoardLayout, nodeKey } from "./use-board-layout";
import { useBoardInteraction } from "./use-board-interaction";
import { Icon } from "../../../lib/icons";
import { relativeTimeLabel, relativeTimeMs } from "../../../lib/format";
import { isDegradedSource, sessionTitleOf, treeSourceLabel } from "../../../lib/tree-labels";
import { useChatStore } from "../../../stores/chat";
import { useTreeBoardStore, zoomPercent } from "../../../stores/tree-board";
import type { TreeChapterView } from "../../../types";

const { t } = useI18n();
const board = useTreeBoardStore();
const chat = useChatStore();

const viewportEl = ref<HTMLElement | null>(null);
const worldEl = ref<HTMLElement | null>(null);
const metrics = useBoardMetrics(worldEl, viewportEl, () => board.scale);
const layout = useBoardLayout(board, metrics.rects);
const interaction = useBoardInteraction({
  board,
  viewportEl,
  rects: metrics.rects,
  viewportSize: metrics.viewport,
  remeasure: metrics.measure,
});

/** 点章节卡 = 定位；该组还没汇聚 ⇒ 给「展开全部标题后汇集」提示（§3.2 第 3 列）。 */
const hint = ref("");
let hintTimer: ReturnType<typeof setTimeout> | undefined;

function onChapterClick(b: TreeChapterView) {
  const g = board.groups.find((x) => x.chapters.some((c) => c.chapterId === b.chapterId));
  if (!g || !layout.panels.value.some((p) => p.key === g.key)) {
    hint.value = t("memory.boardNeedAllTitles");
    clearTimeout(hintTimer);
    hintTimer = setTimeout(() => {
      hint.value = "";
    }, 3000);
    return;
  }
  board.locate(b.chapterId);
}

/** 面板键清单一变就把该读的流读掉（状态 → 效应只留一条通路；重复读走缓存）。 */
watch(
  () => layout.panels.value.map((g) => g.key).join("|"),
  () => {
    for (const g of layout.panels.value) void board.loadStream(g.key);
  },
  { immediate: true },
);

/** 换会话后排了一次「适应」：等 DOM 落位、量准，再缩进视口，然后消费掉旗标。 */
watch(
  () => board.pendingFit,
  async (on) => {
    if (!on) return;
    await nextTick();
    metrics.measure();
    interaction.fit();
    board.consumeFit();
  },
);

/** 内容一变（展开 / 收起 / 换会话 / 换语言）就重测，供连线与包围盒用。 */
watch(
  () =>
    [
      board.groups.length,
      board.openGroups.join(),
      board.openTitles.join(),
      Object.keys(metrics.rects.value).length,
    ].join("|"),
  async () => {
    await nextTick();
    metrics.measure();
  },
);

onBeforeUnmount(() => clearTimeout(hintTimer));

const hasSession = computed(() => board.sessionId !== "" && board.exists);

function timeText(ms: number): string {
  return relativeTimeLabel(relativeTimeMs(ms), (k, p) => t(k, p));
}

const sessionTitle = computed(() =>
  sessionTitleOf(chat.sessions.find((x) => x.id === board.sessionId)?.title, board.sessionId),
);

/** 浮层的时刻 = 投影 mtime（与左栏同一把尺；尾巴计数是 Q16 的 T3，这里不假装有）。 */
const sessionTime = computed(() => {
  const hit = (board.sessions ?? []).find(
    (s) => s.workspace === board.workspace && s.sessionId === board.sessionId,
  );
  return hit ? timeText(hit.updatedMs) : "";
});

const span = (from: number, to: number) => t("memory.boardEventSpan", { from, to });
</script>

<template>
  <div
    ref="viewportEl"
    class="board-viewport relative min-h-0 flex-1 overflow-hidden bg-page"
    tabindex="0"
    :aria-label="t('memory.boardCanvasLabel')"
    @pointerdown="interaction.onPointerDown"
    @pointermove="interaction.onPointerMove"
    @pointerup="interaction.onPointerUp"
    @pointercancel="interaction.onPointerUp"
    @click.capture="interaction.onClickCapture"
    @wheel.prevent="interaction.onWheel"
    @keydown="interaction.onKeyDown"
  >
    <div
      ref="worldEl"
      class="board-world absolute left-0 top-0"
      :class="{ 'is-dragging': board.dragging }"
      :style="{
        width: `${metrics.world.value.w}px`,
        height: `${metrics.world.value.h}px`,
        transform: `translate(${board.tx}px, ${board.ty}px) scale(${board.scale})`,
      }"
    >
      <!-- 网格底：随世界平移缩放，纯装饰（aria-hidden + pointer-events:none） -->
      <div class="board-grid" aria-hidden="true" />

      <!-- 连线：端点全部来自测量表 ⇒ 缩放两极端下不脱节（§二⑯ 验收增项第三条） -->
      <svg class="board-edges" aria-hidden="true" :width="metrics.world.value.w" :height="metrics.world.value.h">
        <path v-for="e in layout.edges.value" :key="e.key" :d="e.d" />
      </svg>

      <template v-if="hasSession">
        <!-- 第 1 列：轮级总结（默认可见 = 树根可见，不是"已展开"） -->
        <div class="board-col" :style="layout.colStyle('summary')">
          <BoardNode
            v-for="g in board.groups"
            :key="nodeKey.summary(g.key)"
            :data-node="nodeKey.summary(g.key)"
            variant="summary"
            :label="g.orphan ? t('memory.boardOrphanGroup') : g.summary"
            :meta="span(g.eventFrom, g.eventTo)"
            :muted="g.orphan"
            :open="board.openGroups.includes(g.key)"
            @click="board.toggleGroup(g.key)"
          />
        </div>

        <!-- 第 2 列：标题条 + 整理中的桩（桩不可点，MB §2.2 第 2 条） -->
        <div
          v-if="layout.titleNodes.value.length > 0 || board.stubs.length > 0"
          class="board-col"
          :style="layout.colStyle('title')"
        >
          <BoardNode
            v-for="b in layout.titleNodes.value"
            :key="nodeKey.title(b.chapterId)"
            :data-node="nodeKey.title(b.chapterId)"
            variant="title"
            :label="b.title"
            :open="board.openTitles.includes(b.chapterId)"
            @click="board.toggleTitle(b.chapterId)"
          />
          <BoardNode
            v-for="b in board.stubs"
            :key="`stub-${b.chapterId}`"
            variant="title"
            :label="t('memory.boardOrganizing')"
            disabled
          />
        </div>

        <!-- 第 3 列：章节卡（点击 = 定位会话流） -->
        <div v-if="layout.chapterNodes.value.length > 0" class="board-col" :style="layout.colStyle('chapter')">
          <BoardNode
            v-for="b in layout.chapterNodes.value"
            :key="nodeKey.chapter(b.chapterId)"
            :data-node="nodeKey.chapter(b.chapterId)"
            variant="chapter"
            :label="b.title"
            :body="b.summary"
            :meta="`${timeText(b.createdAt)} · ${span(b.eventFrom, b.eventTo)}`"
            @click="onChapterClick(b)"
          >
            <template #badges>
              <TreeBadge
                :text="treeSourceLabel(b.source, (k) => t(k))"
                :tone="isDegradedSource(b.source) ? 'warn' : 'neutral'"
              />
              <TreeBadge
                v-if="b.titleEditedByUser || b.summaryEditedByUser"
                :text="t('memory.treeManual')"
              />
            </template>
          </BoardNode>
        </div>

        <!-- 第 4 列：汇聚会话流（逐层展开纪律满足、且量到位次才出现） -->
        <div
          v-for="p in layout.panelViews.value"
          :key="nodeKey.panel(p.group.key)"
          class="board-panel-slot"
          :data-node="nodeKey.panel(p.group.key)"
          :style="layout.panelStyle(p.box)"
        >
          <StreamPanel :group="p.group" :state="board.streamOf(p.group.key)" :height-px="p.box.h" />
        </div>
      </template>
    </div>

    <!-- 三态各归其文案（§二⑩ 空态唯一式：次字色居中，禁止极淡档） -->
    <p
      v-if="board.loading && !hasSession"
      class="board-center text-[length:var(--lw-fs-ui-sm)] text-ink-3"
    >
      {{ t("memory.boardStreamLoading") }}
    </p>
    <p v-else-if="board.error" class="board-center text-[length:var(--lw-fs-ui-sm)] text-danger">
      {{ board.error }}
    </p>
    <p
      v-else-if="!hasSession && board.sessionId"
      class="board-center text-[length:var(--lw-fs-ui-sm)] text-ink-3"
    >
      {{ t("memory.boardNoProjection") }}
    </p>
    <p v-else-if="!hasSession" class="board-center text-[length:var(--lw-fs-ui-sm)] text-ink-3">
      {{ t("memory.boardEmptyProject") }}
    </p>

    <!-- 台面件①：提示行（未汇聚时点章节卡） -->
    <p
      v-if="hint"
      class="board-hint rounded-[var(--lw-r-xs)] bg-elevated px-3 py-1.5 text-[length:var(--lw-fs-ui-xs)] text-ink-2"
    >
      {{ hint }}
    </p>

    <!-- 台面件②：会话浮层（左下）。尾巴计数 = Q16 T3，本刀不假装有。 -->
    <div
      v-if="hasSession"
      class="board-session-chip absolute bottom-4 left-4 flex items-center gap-2 rounded-[var(--lw-r-xs)] bg-elevated px-3 py-1.5 text-[length:var(--lw-fs-ui-xs)] text-ink-2"
    >
      <span class="max-w-[220px] truncate">{{ sessionTitle }}</span>
      <span class="text-ink-4">{{ sessionTime }}</span>
    </div>

    <!-- 台面件③：缩放控件条（右下，− / 读数 / ＋ / 适应，顺序固定） -->
    <div class="board-zoom absolute bottom-4 right-4 flex items-center gap-1 rounded-[var(--lw-r-xs)] bg-elevated p-1">
      <button
        type="button"
        class="board-zoom__btn"
        :aria-label="t('memory.boardZoomOut')"
        @click="board.zoomOut()"
      >
        <Icon name="min" :size="14" />
      </button>
      <span class="board-zoom__read">{{ zoomPercent(board.scale) }}</span>
      <button
        type="button"
        class="board-zoom__btn"
        :aria-label="t('memory.boardZoomIn')"
        @click="board.zoomIn()"
      >
        <Icon name="plus" :size="14" />
      </button>
      <button
        type="button"
        class="board-zoom__btn"
        :aria-label="t('memory.boardZoomFit')"
        @click="interaction.fit()"
      >
        <Icon name="maxDiag" :size="14" />
      </button>
    </div>
  </div>
</template>

<style scoped>
/* 视口 = 不变换的裁剪框；拖拽光标走 §三（grab / grabbing）。 */
.board-viewport {
  cursor: grab;
  outline: none;
}
.board-viewport:active {
  cursor: grabbing;
}
/* 世界层 = 唯一变换件。非拖拽期允许过渡（定位/适应是程序化移动，§二⑯ 只给
   --dur-base + --ease-out）；拖拽期整棵子树硬关过渡——唯一合法写法是
   `transition: none`，`transition-duration: 0s` 与夹带裸时长的多段写法判错。 */
.board-world {
  transform-origin: 0 0;
  transition: transform var(--dur-base) var(--ease-out);
}
.board-world.is-dragging,
.board-world.is-dragging * {
  transition: none;
}
/* 网格底：全站只此一处背景渐变（§二⑯「网格底」行），色位只许 line 族。 */
.board-grid {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background-image: radial-gradient(circle, var(--lw-line-strong) 1px, transparent 1px);
  background-size: 22px 22px;
}
/* 连线：结构线不是盒边框；与网格底同色档（结构线族只留一个色出处）。 */
.board-edges {
  position: absolute;
  inset: 0;
  pointer-events: none;
  overflow: visible;
}
.board-edges path {
  fill: none;
  stroke: var(--lw-line-strong);
  stroke-width: 1;
}
.board-col {
  position: absolute;
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-12);
}
.board-panel-slot {
  position: absolute;
}
.board-center {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.board-hint {
  position: absolute;
  bottom: var(--lw-gap-24);
  left: 50%;
  box-shadow: var(--lw-shadow-pop);
  transform: translateX(-50%);
}
/* 控件条方钮 = --lw-ctl(28) 方 + r-xs + 图标 14（原型 26×24 是自造档，收 28） */
.board-zoom {
  box-shadow: var(--lw-shadow-pop);
}
.board-zoom__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--lw-ctl);
  height: var(--lw-ctl);
  border-radius: var(--lw-r-xs);
  color: var(--lw-ink-2);
  background: transparent;
  transition: background-color var(--dur-fast) var(--ease-standard);
}
.board-zoom__btn:hover {
  background: var(--lw-wash-2);
}
.board-zoom__read {
  min-width: var(--lw-ctl);
  text-align: center;
  font-size: var(--lw-fs-ui-xs);
  color: var(--lw-ink-2);
}
.board-session-chip {
  box-shadow: var(--lw-shadow-pop);
}
</style>
