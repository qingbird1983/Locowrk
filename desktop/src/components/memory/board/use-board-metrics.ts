/**
 * 画布测量件（队列 Q14 第三刀 · §二⑯「世界层」行的执行者）。
 *
 * 为什么必须有真实测量而不是估算：本节写死了两件事——①世界层的**包围盒尺寸由内容算**
 * （原型那颗 `8000×6000` 交互桩判错）；②连线端点锚在**节点左右缘中线**。两者都要
 * 知道"节点现在在哪儿、多高"，而节点高度由文案行数与字号基座决定——只有量出来才是
 * 真数。§二⑯ 验收增项第三条「缩放两极端下连线端点不脱节」抓的正是这件事：只要端点
 * 用估算高度算，缩到 0.5 或放到 2.0 就会飘。
 *
 * ⚠ 坐标系：世界层是唯一被 `transform` 的件，所以 `getBoundingClientRect()` 拿到的是
 * **屏幕像素**，除以当前 scale 才回到世界坐标。这一步不除，pan/zoom 一动手连线就整体
 * 位移（写在这里而不是散进组件，因为这是本件唯一的存在理由）。
 */
import { onBeforeUnmount, onMounted, ref, type Ref } from "vue";
import { WORLD_BLEED_PX, boundsOf, type Rect } from "../../../lib/board-model";

export interface BoardMetrics {
  /** 节点测量表：键 = 元素上的 `data-node` 值（世界坐标）。 */
  rects: Ref<Record<string, Rect>>;
  /** 世界层尺寸（内容包围盒 + 一圈 bleed，不是常数桩）。 */
  world: Ref<{ w: number; h: number }>;
  /** 视口尺寸（未缩放的 CSS 像素，fit 计算的分母）。 */
  viewport: Ref<{ w: number; h: number }>;
  /** 手动重测（布局变化、语言切换、字体就绪都走这里）。 */
  measure: () => void;
}

export function useBoardMetrics(
  worldEl: Ref<HTMLElement | null>,
  viewportEl: Ref<HTMLElement | null>,
  scale: () => number,
): BoardMetrics {
  const rects = ref<Record<string, Rect>>({});
  const world = ref({ w: 0, h: 0 });
  const viewport = ref({ w: 0, h: 0 });

  function measure(): void {
    const el = worldEl.value;
    const box = viewportEl.value;
    if (!el || !box) return;
    const k = scale() || 1;
    const wr = el.getBoundingClientRect();
    const out: Record<string, Rect> = {};
    for (const node of Array.from(el.querySelectorAll<HTMLElement>("[data-node]"))) {
      const key = node.dataset.node;
      if (!key) continue;
      const r = node.getBoundingClientRect();
      out[key] = {
        x: (r.left - wr.left) / k,
        y: (r.top - wr.top) / k,
        w: r.width / k,
        h: r.height / k,
      };
    }
    rects.value = out;
    const list = Object.values(out);
    // 空图 = 世界层退化成"至少铺满视口"，网格才有得画；有内容则包围盒 + 一圈 bleed。
    const bbox = boundsOf(list);
    world.value =
      list.length === 0
        ? { w: box.clientWidth, h: box.clientHeight }
        : { w: bbox.x + bbox.w + WORLD_BLEED_PX, h: bbox.y + bbox.h + WORLD_BLEED_PX };
    viewport.value = { w: box.clientWidth, h: box.clientHeight };
  }

  let ro: ResizeObserver | null = null;
  let raf = 0;

  /** 内容尺寸一变就要重测（展开/收起、换会话、换语言改文案宽度都落在这里）。 */
  function schedule(): void {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(measure);
  }

  onMounted(() => {
    measure();
    if (typeof ResizeObserver === "undefined") return;
    ro = new ResizeObserver(schedule);
    if (worldEl.value) ro.observe(worldEl.value);
    if (viewportEl.value) ro.observe(viewportEl.value);
    void document.fonts?.ready.then(measure); // 字体就绪后行高会变
  });
  onBeforeUnmount(() => {
    cancelAnimationFrame(raf);
    ro?.disconnect();
    ro = null;
  });

  return { rects, world, viewport, measure };
}
