/**
 * 画布三交互（队列 Q14 第三刀 · §二⑯「三交互唯一形态」）。
 *
 * 为什么从 `BoardCanvas.vue` 里分出来：这个 SFC 同时管**几何装配**（列、面板、连线）
 * 与**输入处理**（拖拽 / 滚轮 / 键盘），两件事的改动理由完全不同——调连线不该碰到
 * 键位，调键位不该碰到端点算法。按 §模块规模纪律 的三问自检，"一个文件只做一件事"
 * 在这里不成立 ⇒ 拆成同目录两个 composable，而不是抬基线。
 *
 * ⚠ 拖拽的两条硬要求都落在本件里，改这里前先读它们：
 * ①拖拽期 `board.dragging` 必须为真，画布靠它给整棵子树挂 `transition: none`
 * （跟随指针必须零过渡）；
 * ②拖过 3px 之后这一轮的 click 必须在**捕获相**吃掉——否则拖完松手会误开合节点，
 * 而节点是唯一有写状态的入口（展开态）。
 */
import { ref, type Ref } from "vue";
import { useShellStore } from "../../../stores/shell";
import {
  boundsOf,
  clampScale,
  fitView,
  GRID_STEP_PX,
  ZOOM_STEP,
  zoomAround,
  type Rect,
} from "../../../lib/board-model";
import type { useTreeBoardStore } from "../../../stores/tree-board";

/** 键盘平移一档 = 两个网格步长（派生量，不另造常数）。 */
export const PAN_STEP = GRID_STEP_PX * 2;

export interface BoardInteractionDeps {
  board: ReturnType<typeof useTreeBoardStore>;
  viewportEl: Ref<HTMLElement | null>;
  /** 节点测量表（fit 的包围盒由它算，不是估的）。 */
  rects: Ref<Record<string, Rect>>;
  viewportSize: Ref<{ w: number; h: number }>;
  /** 重测量：程序化移动前必须先量准（第一帧与换语言后都靠它）。 */
  remeasure: () => void;
}

export function useBoardInteraction(deps: BoardInteractionDeps) {
  const shell = useShellStore();
  const drag = ref<{ x: number; y: number } | null>(null);
  let moved = false;

  /** 适应全览：包围盒由**节点测量表**并集算（`boundsOf` 一把尺，不在这里重写
   * min/max），bleed 只是网格余量，不该参与居中。 */
  function fit(): void {
    const box = boundsOf(Object.values(deps.rects.value));
    if (box.w <= 0 || box.h <= 0) return;
    const v = fitView(box, deps.viewportSize.value);
    deps.board.setViewport(v.tx, v.ty, v.scale);
    deps.remeasure();
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    drag.value = { x: e.clientX, y: e.clientY };
    moved = false;
    deps.board.setDrag(true);
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag.value) return;
    const dx = e.clientX - drag.value.x;
    const dy = e.clientY - drag.value.y;
    if (Math.abs(dx) + Math.abs(dy) > 3) moved = true;
    if (!moved) return;
    drag.value = { x: e.clientX, y: e.clientY };
    deps.board.panBy(dx, dy);
  }

  function onPointerUp() {
    drag.value = null;
    deps.board.setDrag(false);
  }

  function onClickCapture(e: MouseEvent) {
    if (!moved) return;
    e.stopPropagation();
    e.preventDefault();
    moved = false;
  }

  /** 滚轮 = 以光标为锚走按钮档（§二⑯ 只钉了按钮步进 25%，没授权第二种尺子）。 */
  function onWheel(e: WheelEvent) {
    const box = deps.viewportEl.value;
    if (!box) return;
    const r = box.getBoundingClientRect();
    const board = deps.board;
    const next = clampScale(board.scale + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
    const v = zoomAround(
      next,
      { x: e.clientX - r.left, y: e.clientY - r.top },
      { tx: board.tx, ty: board.ty, scale: board.scale },
    );
    board.setViewport(v.tx, v.ty, v.scale);
  }

  /** 键盘全路径（鼠标能做到的每个动作都得有键盘路径，画布不许成为唯一入口）。 */
  function onKeyDown(e: KeyboardEvent) {
    const board = deps.board;
    const pan: Record<string, [number, number]> = {
      ArrowLeft: [PAN_STEP, 0],
      ArrowRight: [-PAN_STEP, 0],
      ArrowUp: [0, PAN_STEP],
      ArrowDown: [0, -PAN_STEP],
    };
    const d = pan[e.key];
    if (d) {
      board.panBy(d[0], d[1]);
    } else if (e.key === "+" || e.key === "=") {
      board.zoomIn();
    } else if (e.key === "-") {
      board.zoomOut();
    } else if (e.key === "0") {
      board.zoomReset();
    } else if (e.key === "f" || e.key === "F") {
      fit();
    } else if (e.key === "Escape") {
      shell.setDestination("chat");
    } else {
      return;
    }
    e.preventDefault();
  }

  return { fit, onPointerDown, onPointerMove, onPointerUp, onClickCapture, onWheel, onKeyDown };
}
