<script setup lang="ts">
/**
 * 汇聚会话流面板（队列 Q14 第三刀 · MB §3.2 第 4 列）。
 *
 * 一组一块：某轮总结下**全部**标题都展开后（`streamUnlocked` 判的那一条纪律），
 * 这一轮的完整会话流才在这里汇聚出现——面板不是"另一份原文"，是草图右端那个大盒，
 * 章节卡各有一条贝塞尔边汇入它的左缘。
 *
 * 外壳不另造：走 `BoardNode`（variant = stream）同一只壳件，§二⑯「汇聚面板」行
 * 写的是「外壳与节点同形同档，不新开一档」。内部行也不自造：小节头徽章走
 * `TreeBadge`、原文行走 `OriginalRows`，与「目录」档共用同两件。
 *
 * 失败语义分两层（§2.3 落地时定死的那条）：整组读不出来 = `state.error` 一条
 * 行内错误；单节切片越界 = 那一节自己报错（`section.error`），其余照常——
 * 两种"读不到"都不能演成"没有内容"。
 */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import BoardNode from "./BoardNode.vue";
import OriginalRows from "../OriginalRows.vue";
import TreeBadge from "../TreeBadge.vue";
import { isDegradedSource, treeSourceLabel } from "../../../lib/tree-labels";
import type { BoardGroup } from "../../../lib/board-model";
import { useTreeBoardStore, type StreamState } from "../../../stores/tree-board";

const props = defineProps<{
  group: BoardGroup;
  state: StreamState;
  /** 定高（px）：该总结子树的纵向跨度，由画布测量给——不是估算值。 */
  heightPx: number;
}>();

const { t } = useI18n();
const board = useTreeBoardStore();

const scroller = ref<HTMLElement | null>(null);
const flashId = ref("");
let flashTimer: ReturnType<typeof setTimeout> | undefined;

const sections = computed(() => props.state.view?.sections ?? []);

/** 点章节卡 = 定位：面板内滚到那一节 + 闪一次（一次性动画，§二⑯ 明确它
 * 不进常驻动画白名单）。tick 自增 ⇒ 连点同一章也重放。 */
watch(
  () => board.locateTick,
  () => {
    const id = board.locateChapterId;
    if (!id || !sections.value.some((s) => s.chapterId === id)) return;
    const box = scroller.value;
    const target = box?.querySelector<HTMLElement>(`[data-sec="${id}"]`);
    if (!box || !target) return;
    box.scrollTop = Math.max(0, target.offsetTop - box.offsetTop);
    flashId.value = id;
    clearTimeout(flashTimer);
    // 闪两轮（`var(--dur-slow)` ×2）后摘掉类，下次点击才重放得动
    flashTimer = setTimeout(() => {
      flashId.value = "";
    }, 700);
  },
);

onBeforeUnmount(() => clearTimeout(flashTimer));
</script>

<template>
  <BoardNode variant="stream" :label="''" :height-px="heightPx">
    <template #flow>
      <!-- 滚轮落在这里 = 滚面板，不拖画布（§二⑯ pan 只属于视口） -->
      <div ref="scroller" class="board-flow scroll-y" @wheel.stop>
        <p v-if="state.loading" class="board-flow__note">
          {{ t("memory.boardStreamLoading") }}
        </p>
        <p v-else-if="state.error" class="board-flow__note is-bad">
          {{ t("memory.boardStreamFailed", { reason: state.error }) }}
        </p>
        <p v-else-if="sections.length === 0" class="board-flow__note">
          {{ t("memory.boardStreamEmpty") }}
        </p>
        <template v-else>
          <section
            v-for="sec in sections"
            :key="sec.chapterId"
            :data-sec="sec.chapterId"
            class="board-sec"
            :class="{ 'is-flash': flashId === sec.chapterId }"
          >
            <header class="board-sec__head">
              <span class="board-sec__title">{{ sec.title }}</span>
              <TreeBadge
                :text="treeSourceLabel(sec.source, (k) => t(k))"
                :tone="isDegradedSource(sec.source) ? 'warn' : 'neutral'"
                :title="sec.source"
              />
              <TreeBadge v-if="sec.edited" :text="t('memory.treeManual')" />
            </header>
            <OriginalRows v-if="sec.rows.length > 0" :rows="sec.rows" />
            <p v-else-if="sec.error" class="board-flow__note is-bad">
              {{ t("memory.boardSectionFailed", { reason: sec.error }) }}
            </p>
            <p v-else class="board-flow__note">
              {{ t("memory.boardSectionEmpty") }}
            </p>
          </section>
        </template>
      </div>
    </template>
  </BoardNode>
</template>

<style scoped>
/* 面板内边距走 --lw-gap-*（§二⑬）；字号全落相对制（§二⑯ 字号条款）。 */
.board-flow {
  height: 100%;
  overflow-y: auto;
  padding: var(--lw-gap-10) var(--lw-gap-16);
}
.board-sec {
  border-radius: var(--lw-r-xs);
  padding: var(--lw-gap-6) var(--lw-gap-8);
}
.board-sec + .board-sec {
  margin-top: var(--lw-gap-10);
}
.board-sec__head {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lw-gap-6);
  align-items: center;
  margin-bottom: var(--lw-gap-4);
}
.board-sec__title {
  min-width: 0;
  font-size: var(--lw-fs-ui-base);
  font-weight: 600;
  color: var(--lw-ink-1);
}
.board-flow__note {
  font-size: var(--lw-fs-ui-sm);
  color: var(--lw-ink-4);
}
.board-flow__note.is-bad {
  color: var(--lw-danger);
}
/* 定位闪烁 = 一次性（`2` 轮后自然停；§二⑯ 明确它不进 §二① 常驻白名单） */
.board-sec.is-flash {
  animation: lw-board-flash var(--dur-slow) var(--ease-standard) 2;
}
@keyframes lw-board-flash {
  0%,
  100% {
    background: transparent;
  }
  50% {
    background: var(--lw-wash-3);
  }
}
</style>
