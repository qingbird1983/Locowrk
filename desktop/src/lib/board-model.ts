/**
 * 记忆树看板的纯模型层（队列 Q14 第三刀 · MB《Memory Tree Board Design v1》§2/§3）。
 *
 * 为什么单开一个 lib 而不是写进组件：①这一层是**可预测几何**——连线的端点、
 * 适应视口的包围盒、逐层展开的纪律判定，全都必须是纯函数才能有用例钉住
 * （Q11 起的测试面 = store/纯函数 + 组件双档渲染）；②`§二⑯` 那句「节点定宽的
 * 唯一理由 = 连线端点几何可预测」意味着这些数**只许有一个出处**——一旦在 CSS
 * 里再抄一份 px，缩放两极端下端点就会脱节（那是 §二⑯ 验收增项第三条）。
 * 所以这里的常量既喂布局（内联样式），也喂连线路径计算，CSS 里不出现任何一个。
 *
 * 数值出处 = 《Front-end Design Guidelines》§八 勾⑧⑨⑩ 的缺省甲案
 * （勾⑨ A：总结 250 / 标题 240 / 章节 270 / 汇聚面板 460，列 x 基线
 * 32 / 350 / 650 / 985，源 `memory-board.html:145-148`）。**原型是 UI 参考不是
 * 合同**，被本合同采纳的只有这几枚尺寸；稿 §3 与原型里其余的常量（世界层
 * 8000×6000、缩放钮 26×24、hover 变描边）在 §二⑯ 的订正清单里已被判错，本文件
 * 一个都不搬。
 */
import type { TreeChapterView, TreeChaptersView } from "../types";

// ---------------------------------------------------------------- 尺寸（勾⑨ 甲案）

/** 四类节点的定宽（列间空隙 = 后一列基线 − 本列基线 − 本列宽，不另给 gap）。 */
export const NODE_W = {
  summary: 250,
  title: 240,
  chapter: 270,
  stream: 460,
} as const;

/** 节点形态 = 定宽表的键（不另列枚举：形态清单与尺寸表分两处写必漂）。 */
export type BoardVariant = keyof typeof NODE_W;

/**
 * 四列的 x 基线（世界层坐标，未缩放）。列宽与基线同源 ⇒ 端点算得出、不漂。
 * `satisfies Record<BoardVariant, number>`：少一列或多一列都在类型面直接编译失败，
 * 不必靠用例去撞。
 */
export const COLUMN_X = {
  summary: 32,
  title: 350,
  chapter: 650,
  stream: 985,
} as const satisfies Record<BoardVariant, number>;

/** 网格点阵步长（勾⑧ 甲案 22px；纯装饰，不进布局计算）。 */
export const GRID_STEP_PX = 22;

// ---------------------------------------------------------------- 缩放（勾⑩ 甲案）

export const ZOOM_MIN = 0.5;
export const ZOOM_MAX = 2.0;
/**
 * 按钮步进 = **25 个百分点**（0.50 → 0.75 → 1.00 → 1.25 …），读数直接改，
 * 不做数字滚动动画。选加法而不是「当前值的 25%」：乘性档从 0.5 下界起步会算出
 * 0.625 / 0.781 这类读不出、且永远按不到 100% 的数，而 `1:1` 复位要求 1 可达。
 */
export const ZOOM_STEP = 0.25;

export function clampScale(v: number): number {
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, v));
}

// ---------------------------------------------------------------- 分组与归组

/** 一组的键：轮级总结用 `from:to`，未归总结的虚拟组用 `"orphan"`。
 * 这两个形状就是后端 `tree_stream` 的 group 参数——前端不另造第三种。 */
export const ORPHAN_GROUP = "orphan";

export interface BoardGroup {
  key: string;
  /** 总结节点文本；未归组为空串（它没有父节点，样式弱化）。 */
  summary: string;
  createdAt: number;
  eventFrom: number;
  eventTo: number;
  /** 组内成员（已定稿章节，后端 `chapter_ids` 给什么就是什么）。 */
  chapters: TreeChapterView[];
  orphan: boolean;
}

/**
 * 把章节清单装配成组。
 *
 * ⚠ 归组只走 `summary.chapterIds` 的**集合成员**，不重算事件区间包含——区间
 * 判据的权威源在后端 `chapters_in`，在这里再 `eventFrom >= … && eventTo <= …`
 * 一次就是第二个出处（§二⑯ 与本仓 §11 同一条纪律）。
 *
 * 占位章节（pending）不进任何组：MB §2.2 第 2 条要它在标题列显示「整理中…」桩，
 * 那是 `pendingStubs()` 的活；会话流里它一个字都不出现。
 */
export function buildGroups(view: TreeChaptersView): BoardGroup[] {
  const byId = new Map(view.chapters.map((b) => [b.chapterId, b]));
  const claimed = new Set<string>();
  const groups: BoardGroup[] = [];
  for (const s of view.summaries) {
    const members: TreeChapterView[] = [];
    for (const id of s.chapterIds) {
      const b = byId.get(id);
      if (!b || b.pending) continue;
      members.push(b);
      claimed.add(id);
    }
    groups.push({
      key: `${s.eventFrom}:${s.eventTo}`,
      summary: s.text,
      createdAt: s.createdAt,
      eventFrom: s.eventFrom,
      eventTo: s.eventTo,
      chapters: members,
      orphan: false,
    });
  }
  const orphans = view.chapters.filter(
    (b) => !b.pending && !claimed.has(b.chapterId),
  );
  if (orphans.length > 0) {
    groups.push({
      key: ORPHAN_GROUP,
      summary: "",
      createdAt: 0,
      eventFrom: orphans[0]!.eventFrom,
      eventTo: orphans[orphans.length - 1]!.eventTo,
      chapters: orphans,
      orphan: true,
    });
  }
  return groups;
}

/** 整理中的桩（标题列的「整理中…」，不可展开）。 */
export function pendingStubs(view: TreeChaptersView): TreeChapterView[] {
  return view.chapters.filter((b) => b.pending);
}

/**
 * 逐层展开纪律（MB §3.2 v1.2 定死的那一条）：一组里**全部**成员的标题都展开后，
 * 汇聚面板才出现；任一收起 ⇒ 立即不收（面板随之收起）。
 *
 * 空组（孤儿组没有成员）返回 false——没有内容可汇，硬开一块空面板 = 演出来一个
 * 不存在的小节。
 */
export function streamUnlocked(group: BoardGroup, openTitles: ReadonlySet<string>): boolean {
  return group.chapters.length > 0 && group.chapters.every((b) => openTitles.has(b.chapterId));
}

// ---------------------------------------------------------------- 日期分组（左栏）

/** 左栏会话列表的日期档（MB §3.1：今天 / 昨天 / 本周 / 更早）。 */
export type DayBucket = "today" | "yesterday" | "week" | "earlier";

const DAY_MS = 86_400_000;

/** 按本地日界分档——不能用 `now - updated < 24h` 冒充「今天」（昨天 23 点也满足）。 */
export function dayBucketOf(updatedMs: number, nowMs: number): DayBucket {
  const startOfToday = new Date(nowMs).setHours(0, 0, 0, 0);
  const t = new Date(updatedMs).setHours(0, 0, 0, 0);
  if (t >= startOfToday) return "today";
  if (t >= startOfToday - DAY_MS) return "yesterday";
  if (t >= startOfToday - 6 * DAY_MS) return "week";
  return "earlier";
}

/** 分档顺序固定（今天 → 昨天 → 本周 → 更早），空档不出现。 */
export function bucketSessions<T extends { updatedMs: number }>(
  items: T[],
  nowMs: number,
): { bucket: DayBucket; items: T[] }[] {
  const order: DayBucket[] = ["today", "yesterday", "week", "earlier"];
  const out: { bucket: DayBucket; items: T[] }[] = [];
  for (const bucket of order) {
    const hit = items.filter((x) => dayBucketOf(x.updatedMs, nowMs) === bucket);
    if (hit.length > 0) out.push({ bucket, items: hit });
  }
  return out;
}

// ---------------------------------------------------------------- 几何（连线 / 视口）

export interface Point {
  x: number;
  y: number;
}

/**
 * 三次贝塞尔（水平出入）：两端沿 x 方向各让出跨度的一半。
 * 返回值只描述形状，颜色/线宽由 CSS 走 `§二⑯`（连线 = `--lw-line-strong` 1px）。
 */
export function bezierPath(from: Point, to: Point): string {
  const dx = Math.max(24, Math.abs(to.x - from.x) / 2) * Math.sign(to.x - from.x || 1);
  return (
    `M ${from.x} ${from.y} ` +
    `C ${from.x + dx} ${from.y}, ${to.x - dx} ${to.y}, ${to.x} ${to.y}`
  );
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function boundsOf(rects: Rect[]): Rect {
  if (rects.length === 0) return { x: 0, y: 0, w: 0, h: 0 };
  const x0 = Math.min(...rects.map((r) => r.x));
  const y0 = Math.min(...rects.map((r) => r.y));
  const x1 = Math.max(...rects.map((r) => r.x + r.w));
  const y1 = Math.max(...rects.map((r) => r.y + r.h));
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

/** 适应全览：整幅内容等比缩进视口（留 pad），读数钳进勾⑩ 的范围。 */
export function fitView(
  bounds: Rect,
  viewport: { w: number; h: number },
  pad = 32,
): { scale: number; tx: number; ty: number } {
  if (bounds.w <= 0 || bounds.h <= 0 || viewport.w <= 0 || viewport.h <= 0) {
    return { scale: 1, tx: 0, ty: 0 };
  }
  const raw = Math.min(
    (viewport.w - pad * 2) / bounds.w,
    (viewport.h - pad * 2) / bounds.h,
  );
  const scale = clampScale(raw);
  return {
    scale,
    tx: (viewport.w - bounds.w * scale) / 2 - bounds.x * scale,
    ty: (viewport.h - bounds.h * scale) / 2 - bounds.y * scale,
  };
}

/** 世界层在内容包围盒外多铺的一圈（让平移时网格不断）。
 * 取 4 倍网格步长 = 一个可见的整数格数，**不是**原型那颗 `8000×6000` 交互桩
 * （§二⑯「世界层」行禁止照搬：包围盒由内容算，这一圈只是内容的函数）。 */
export const WORLD_BLEED_PX = GRID_STEP_PX * 4;

/** 内容之外那一圈的偏移：列与行都从这个原点起算（测量与布局共用一把尺）。 */
export const WORLD_ORIGIN_OFFSET_PX = WORLD_BLEED_PX;

/** 节点左右缘中线（连线端点锚位，§二⑯「连线」行）。 */
export function anchorLeft(r: Rect): Point {
  return { x: r.x, y: r.y + r.h / 2 };
}

export function anchorRight(r: Rect): Point {
  return { x: r.x + r.w, y: r.y + r.h / 2 };
}

/** 以某点为锚缩放（滚轮档）：缩放前后该点落在屏幕同一位置。 */
export function zoomAround(
  nextScale: number,
  anchor: Point,
  view: { tx: number; ty: number; scale: number },
): { scale: number; tx: number; ty: number } {
  const scale = clampScale(nextScale);
  const k = scale / view.scale;
  return { scale, tx: anchor.x - (anchor.x - view.tx) * k, ty: anchor.y - (anchor.y - view.ty) * k };
}
