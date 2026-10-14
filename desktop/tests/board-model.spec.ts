// Q14 第三刀：看板纯模型层（`src/lib/board-model.ts`）。
//
// 这一层是画布唯一"能在不渲染的情况下被钉住"的部分，也是 §二⑯ 两句硬话的执行者：
// ①**归组判据只许一个出处**（后端 `chapters_in` 算好的 `chapterIds`，前端不许再按
// 事件区间反查一遍）；②**尺寸常量只许一个出处**（列基线与定宽同时喂布局与连线端点，
// 抄进 CSS 就是第二个出处，缩放两极端下端点会脱节 = §二⑯ 验收增项第三条）。
import { describe, expect, it } from "vitest";
import {
  boundsOf,
  bucketSessions,
  buildGroups,
  clampScale,
  COLUMN_X,
  dayBucketOf,
  fitView,
  GRID_STEP_PX,
  NODE_W,
  ORPHAN_GROUP,
  pendingStubs,
  streamUnlocked,
  WORLD_BLEED_PX,
  ZOOM_MAX,
  ZOOM_MIN,
  ZOOM_STEP,
  anchorLeft,
  anchorRight,
  bezierPath,
  zoomAround,
} from "../src/lib/board-model";
import {
  MEMBERSHIP_WINS_OVER_INTERVAL,
  NO_SUMMARY,
  ONE_RUN_TWO_CHAPTERS,
  WITH_PLACEHOLDER,
} from "./board-fixture";

const ids = (list: { chapters: { chapterId: string }[] }) =>
  list.chapters.map((b) => b.chapterId);

describe("buildGroups：归组只认 chapterIds 的集合成员", () => {
  it("区间在总结之内但没被点名 = 不归这组；被点名但区间在外 = 照样归这组", () => {
    const groups = buildGroups(MEMBERSHIP_WINS_OVER_INTERVAL);
    const run = groups.find((g) => g.key === "0:8");
    expect(run?.chapters.map((b) => b.chapterId)).toEqual(["b1", "b9"]);
    // 这一条就是「前端不再按区间反查」的证明：b3 的 2–5 完整落在 0–8 之内，
    // 若这里按 `eventFrom >= from && eventTo <= to` 重算，它会出现在组内。
    expect(run?.chapters.some((b) => b.chapterId === "b3")).toBe(false);
    const orphan = groups.find((g) => g.key === ORPHAN_GROUP);
    expect(ids(orphan ?? { chapters: [] })).toEqual(["b3"]);
  });

  it("占位章节被点名也不进组，只出「整理中…」桩（MB §2.2 第 2 条）", () => {
    const groups = buildGroups(WITH_PLACEHOLDER);
    const run = groups.find((g) => g.key === "0:8");
    expect(ids(run ?? { chapters: [] })).toEqual(["b1", "b9"]);
    expect(pendingStubs(WITH_PLACEHOLDER).map((b) => b.chapterId)).toEqual(["b7"]);
    // 桩也不能落进未归组：否则会话流里凭空多一节空原文
    const orphan = groups.find((g) => g.key === ORPHAN_GROUP);
    expect(orphan?.chapters.some((b) => b.chapterId === "b7")).toBe(false);
  });

  it("没有总结的会话 = 只有一枚未归组，且它的区间取首末章节（不是造出来的 0:0）", () => {
    const groups = buildGroups(NO_SUMMARY);
    expect(groups).toHaveLength(1);
    expect(groups[0]?.key).toBe(ORPHAN_GROUP);
    expect(groups[0]?.orphan).toBe(true);
    expect(groups[0]?.eventFrom).toBe(0);
    expect(groups[0]?.eventTo).toBe(8);
    expect(groups[0]?.summary).toBe("");
  });

  it("组键就是后端 stream 的 group 参数：总结用 from:to，未归用 orphan", () => {
    const [one] = buildGroups(ONE_RUN_TWO_CHAPTERS);
    expect(one?.key).toBe("0:8");
    expect(buildGroups(NO_SUMMARY)[0]?.key).toBe("orphan");
  });
});

describe("streamUnlocked：逐层展开纪律（§3.2 v1.2 定死）", () => {
  const group = buildGroups(ONE_RUN_TWO_CHAPTERS)[0]!;
  it("少一个标题展开就不许汇聚出现", () => {
    expect(streamUnlocked(group, new Set(["b1"]))).toBe(false);
    expect(streamUnlocked(group, new Set())).toBe(false);
  });
  it("全部标题展开 = 出现", () => {
    expect(streamUnlocked(group, new Set(["b1", "b2"]))).toBe(true);
  });
  it("空组返回 false：没有内容可汇，硬开一块面板 = 演一个不存在的小节", () => {
    expect(streamUnlocked({ ...group, chapters: [] }, new Set(["b1", "b2"]))).toBe(false);
  });
});

describe("日期分档（左栏）：按本地日界，不按 24 小时减法", () => {
  const now = new Date(2026, 9, 15, 10, 0, 0).getTime(); // 10 月 15 日上午 10 点
  const sameYesterdayLate = new Date(2026, 9, 14, 23, 0, 0).getTime();
  it("昨天 23 点不满 24 小时，也不能算「今天」", () => {
    expect(now - sameYesterdayLate).toBeLessThan(86_400_000);
    expect(dayBucketOf(sameYesterdayLate, now)).toBe("yesterday");
  });
  it("四档边界（今天 / 昨天 / 本周 = 六天内 / 更早）", () => {
    expect(dayBucketOf(now, now)).toBe("today");
    expect(dayBucketOf(new Date(2026, 9, 9, 8).getTime(), now)).toBe("week");
    expect(dayBucketOf(new Date(2026, 9, 8, 8).getTime(), now)).toBe("earlier");
  });
  it("空档不出现，出现顺序固定", () => {
    const items = [
      { updatedMs: new Date(2026, 9, 15, 9).getTime() },
      { updatedMs: new Date(2026, 9, 14, 9).getTime() },
    ];
    expect(bucketSessions(items, now).map((b) => b.bucket)).toEqual(["today", "yesterday"]);
  });
});

describe("几何：连线端点 / 包围盒 / 适应 / 锚点缩放", () => {
  it("贝塞尔首尾就是两个端点（端点脱节 = §二⑯ 验收增项第三条）", () => {
    const d = bezierPath({ x: 10, y: 20 }, { x: 110, y: 60 });
    expect(d.startsWith("M 10 20 ")).toBe(true);
    expect(d.endsWith(", 110 60")).toBe(true);
  });
  it("左右缘中线：宽 250 高 60 的节点，出点 = (260, 50)，入点 = (350, 50)", () => {
    const r = { x: 10, y: 20, w: 250, h: 60 };
    expect(anchorRight(r)).toEqual({ x: 260, y: 50 });
    expect(anchorLeft({ x: 350, y: 20, w: 240, h: 60 })).toEqual({ x: 350, y: 50 });
  });
  it("空表不炸（第一帧还没测到任何节点）", () => {
    expect(boundsOf([])).toEqual({ x: 0, y: 0, w: 0, h: 0 });
    expect(fitView(boundsOf([]), { w: 800, h: 600 })).toEqual({ scale: 1, tx: 0, ty: 0 });
  });
  it("适应全览：内容中心落视口中心，且读数钳进勾⑩ 的范围", () => {
    const bbox = boundsOf([{ x: 0, y: 0, w: 4000, h: 400 }]);
    const v = fitView(bbox, { w: 800, h: 600 });
    expect(v.scale).toBe(ZOOM_MIN); // 4000 宽塞进 800 需要 0.2 ⇒ 钳到下限
    const cx = bbox.x + bbox.w / 2;
    expect(v.tx + cx * v.scale).toBeCloseTo(400, 6);
  });
  it("1:1 复位与按钮档：0.5 起步每次 25 个百分点，1.0 与 2.0 都按得到", () => {
    // 这一条钉死「加法档」这个读数（勾⑩ 甲案 + §二⑯ 键盘 `0` 回 100%）。
    // 乘性档（当前值的 25%）从 0.5 出发会算出 0.625 / 0.781，永远按不到 100%。
    let s = clampScale(ZOOM_MIN - ZOOM_STEP); // 故意从范围外进来：先钳到下限
    const steps = [s];
    for (let i = 0; i < 6; i++) {
      s = clampScale(s + ZOOM_STEP);
      steps.push(s);
    }
    expect(steps).toEqual([0.5, 0.75, 1, 1.25, 1.5, 1.75, 2]);
    expect(steps).toContain(1); // `0` 键的 100% 必须落在档位序列里
    expect(clampScale(9)).toBe(ZOOM_MAX);
    expect(clampScale(0.1)).toBe(ZOOM_MIN);
  });
  it("以光标为锚缩放：光标下面那个**世界点**在缩放前后仍落在同一屏幕位", () => {
    // anchor 是视口相对的屏幕点（`onWheel` 传的就是这个口径），不是世界坐标。
    const anchor = { x: 300, y: 200 };
    const view = { tx: 40, ty: 60, scale: 1 };
    const next = zoomAround(1.5, anchor, view);
    const worldUnderAnchor = (anchor.x - view.tx) / view.scale;
    expect(next.scale).toBe(1.5);
    expect(worldUnderAnchor * next.scale + next.tx).toBeCloseTo(anchor.x, 6);
  });
});

describe("尺寸常量表：§二⑯ 的「只许一个出处」", () => {
  it("四列基线与四档定宽一一对应（少一列或多一列都算形状变了）", () => {
    expect(Object.keys(COLUMN_X)).toEqual(Object.keys(NODE_W));
    expect(COLUMN_X).toEqual({ summary: 32, title: 350, chapter: 650, stream: 985 });
    expect(NODE_W).toEqual({ summary: 250, title: 240, chapter: 270, stream: 460 });
  });
  it("列间不重叠：后一列基线 ≥ 前一列基线 + 前一列宽", () => {
    const cols = Object.keys(COLUMN_X) as (keyof typeof COLUMN_X)[];
    for (let i = 1; i < cols.length; i++) {
      const prev = cols[i - 1]!;
      expect(COLUMN_X[cols[i]!]).toBeGreaterThanOrEqual(COLUMN_X[prev] + NODE_W[prev]);
    }
  });
  it("网格余量是步长的整数倍（WORLD_BLEED 由 GRID_STEP 派生，不是第二个数）", () => {
    expect(WORLD_BLEED_PX % GRID_STEP_PX).toBe(0);
  });
});
