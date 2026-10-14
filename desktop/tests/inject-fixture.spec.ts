// 夹具自证（Q11 立的一条新纪律）：**测试喂给界面的数据必须讲后端真会讲的故事**。
// 本仓在这上面吃过两次亏——一次是夹具时间基准与生产不同源（差 3360 天，判的其实是夹具），
// 一次是字段名照想象编（测试全绿而真跑恒为空串）。所以夹具不许"看着像"，要能被重算。
//
// ⚠ **这段算式不是生产的第二份实现**：真身 = `memory_learn/explain.rs::share_permille`
// 与 `memory_learn/signals.rs` 的千分比口径（前端 `src/` 里出现同类算式由形状判据判红）。
// 这里重算的对象只有 `inject-fixture.ts` 里那三行数字，用来证明它自洽。
import { describe, expect, it } from "vitest";
import { VIEW_WITH_ROW } from "./inject-fixture";

const WEIGHTS = [400, 250, 200, 150]; // 千分比（内核 LEARN_WEIGHT_DEFAULT 的缺省档）
const SIGNALS = [900, 500, 300, 100]; // 千分比四信号

/** 生产口径：`share_i = w_i·s_i·1000/total` 整除，**余量归最大一档**，四档凑满 1000。 */
function shares(weights: number[], signals: number[]): { share: number[]; total: number } {
  const parts = weights.map((w, i) => Math.floor((w * signals[i]! * 1000) / totalOf(weights, signals)));
  const total = totalOf(weights, signals);
  const sum = parts.reduce((a, b) => a + b, 0);
  const biggest = parts.indexOf(Math.max(...parts));
  parts[biggest] += 1000 - sum;
  return { share: parts, total };
}

function totalOf(weights: number[], signals: number[]): number {
  return weights.reduce((acc, w, i) => acc + w * signals[i]!, 0);
}

describe("注入回执夹具的自洽性", () => {
  it("total = 加权和，与 fixture 里的读数同值", () => {
    expect(totalOf(WEIGHTS, SIGNALS)).toBe(VIEW_WITH_ROW.explain.rows[0]!.total);
    expect(VIEW_WITH_ROW.explain.weights).toEqual(WEIGHTS);
  });

  it("四档占比按生产口径重算 = fixture 的 share，且四档相加恰为 1000", () => {
    const { share } = shares(WEIGHTS, SIGNALS);
    expect(share).toEqual(VIEW_WITH_ROW.explain.rows[0]!.share);
    expect(share.reduce((a, b) => a + b, 0)).toBe(1000);
  });

  it("比率读数各归其义：yield = yielded/injected，null 与 0 不是一回事", () => {
    const o = VIEW_WITH_ROW.overview;
    expect(o.injection_yield).toBe(o.yielded_keys / o.injected_keys);
    expect(o.store_empty_ratio).not.toBe(0); // 0 = 「注入过但全空」，null = 「没注入过」
  });
});
