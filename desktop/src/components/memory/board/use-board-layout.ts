/**
 * 画布几何装配（队列 Q14 第三刀 · §二⑯「节点 / 连线 / 汇聚面板」三行的判定处）。
 *
 * 与 `use-board-interaction.ts` 分家的理由：本件只回答"什么该出现在第几列、边连到
 * 哪儿"，全部输入是**状态 + 测量表**，一个输入事件都不碰。两件事的改动理由互不重叠
 * （调连线不该碰到键位），而 §模块规模纪律 的三问自检在合写的版本里不成立。
 *
 * ⚠ 这里不许出现第二条归组判据：列内容由 `store.panelGroups` 与 `group.chapters` 给
 * （后端的 `chapter_ids` 是唯一出处），本件只按 `openGroups` / `openTitles` 两把展开
 * 尺筛可见性，绝不按 `eventFrom/eventTo` 反查谁属于谁。
 */
import { computed, type Ref } from "vue";
import {
  anchorLeft,
  anchorRight,
  bezierPath,
  boundsOf,
  COLUMN_X,
  NODE_W,
  WORLD_ORIGIN_OFFSET_PX,
  type BoardVariant,
  type BoardGroup,
  type Rect,
} from "../../../lib/board-model";
import type { TreeChapterView } from "../../../types";
import type { useTreeBoardStore } from "../../../stores/tree-board";

/** 节点/面板的测量键：列前缀 + 身份（连线端点与包围盒都按它取）。 */
export const nodeKey = {
  summary: (groupKey: string) => `s:${groupKey}`,
  title: (chapterId: string) => `t:${chapterId}`,
  chapter: (chapterId: string) => `c:${chapterId}`,
  panel: (groupKey: string) => `p:${groupKey}`,
};

export interface BoardEdge {
  key: string;
  d: string;
}

export function useBoardLayout(
  board: ReturnType<typeof useTreeBoardStore>,
  rects: Ref<Record<string, Rect>>,
) {
  const openGroups = computed<BoardGroup[]>(() =>
    board.groups.filter((g) => board.openGroups.includes(g.key)),
  );
  /** 第 2 列 = 已展开总结的标题条（占位桩由画布排在同一列末尾）。 */
  const titleNodes = computed<TreeChapterView[]>(() =>
    openGroups.value.flatMap((g) => g.chapters),
  );
  /** 第 3 列 = 已展开标题的章节卡。 */
  const chapterNodes = computed<TreeChapterView[]>(() =>
    titleNodes.value.filter((b) => board.openTitles.includes(b.chapterId)),
  );
  /** 第 4 列的可见面板（两层条件已在 store 判过一次）。 */
  const panels = computed<BoardGroup[]>(() => board.panelGroups);

  /**
   * 面板落位：纵向跨度 = 该组章节卡测量并集（§3.2 汇聚语义）。
   * 两种"还没量到"都跳过：一个矩形都没有、或并集高度 ≤ 0 —— 面板宁可晚一帧出现，
   * 也不给一个 `height: 0` 的空壳（空壳在验收增项第二条里就是"残影"）。
   */
  const panelViews = computed<{ group: BoardGroup; box: Rect }[]>(() => {
    const out: { group: BoardGroup; box: Rect }[] = [];
    for (const g of panels.value) {
      const box = boundsOf(
        g.chapters
          .map((b) => rects.value[nodeKey.chapter(b.chapterId)])
          .filter((r): r is Rect => Boolean(r)),
      );
      if (box.h <= 0) continue;
      out.push({ group: g, box: { x: COLUMN_X.stream, y: box.y, w: NODE_W.stream, h: box.h } });
    }
    return out;
  });

  /** 三段汇入边：总结→标题条、标题条→章节卡、章节卡→面板左缘。 */
  const edges = computed<BoardEdge[]>(() => {
    const r = rects.value;
    const out: BoardEdge[] = [];
    const push = (key: string, fromKey: string, toKey: string) => {
      const from = r[fromKey];
      const to = r[toKey];
      if (!from || !to) return;
      out.push({ key, d: bezierPath(anchorRight(from), anchorLeft(to)) });
    };
    for (const g of openGroups.value) {
      for (const b of g.chapters) {
        push(
          `e:s-${g.key}-${b.chapterId}`,
          nodeKey.summary(g.key),
          nodeKey.title(b.chapterId),
        );
      }
    }
    for (const b of chapterNodes.value) {
      push(`e:t-${b.chapterId}`, nodeKey.title(b.chapterId), nodeKey.chapter(b.chapterId));
    }
    for (const p of panelViews.value) {
      for (const b of p.group.chapters) {
        push(`e:c-${b.chapterId}`, nodeKey.chapter(b.chapterId), nodeKey.panel(p.group.key));
      }
    }
    return out;
  });

  /** 列内联样式：x 基线与定宽全取 board-model，CSS 里一个 px 都不抄。 */
  function colStyle(variant: Exclude<BoardVariant, "stream">) {
    return {
      left: `${WORLD_ORIGIN_OFFSET_PX + COLUMN_X[variant]}px`,
      top: `${WORLD_ORIGIN_OFFSET_PX}px`,
      width: `${NODE_W[variant]}px`,
    };
  }

  function panelStyle(rect: Rect) {
    return {
      left: `${rect.x}px`,
      top: `${rect.y}px`,
      width: `${rect.w}px`,
      height: `${rect.h}px`,
    };
  }

  return {
    openGroups,
    titleNodes,
    chapterNodes,
    panels,
    panelViews,
    edges,
    colStyle,
    panelStyle,
  };
}
