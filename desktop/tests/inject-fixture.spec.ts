// 夹具自证（Q11 立的一条纪律）：**测试喂给界面的数据必须讲后端真会讲的故事**。
// 本仓在这上面吃过两次亏——一次是夹具时间基准与生产不同源（差 3360 天，判的其实是夹具），
// 一次是字段名照想象编（测试全绿而真跑恒为空串）。所以夹具不许"看着像"，要能被重算。
//
// ⚠ 这个文件第一版重算的是四信号占比（`totalOf` / `shares` 那一套千分比口径，真身 =
// `memory_learn/explain.rs::share_permille` 与 `signals.rs`）。那批数字随学习档在
// **S3 刀③b 整族退役** ⇒ 前端再没有占比可校；"算式不许在前端重抄一份"那条形状判据
// 也一并失去对象。**留下的仍是同一条纪律，只是换了校核对象**：overview 那五个数
// 必须互相自洽，而且"读数背后必须是整数次计数"这一维必须判得住。
import { describe, expect, it } from "vitest";
import { VIEW, VIEW_EMPTY } from "./inject-fixture";

describe("注入观测夹具的自洽性", () => {
  it("yield = yielded_keys / injected_keys，两个读数同源于键位集合而不是次数", () => {
    const o = VIEW.overview;
    expect(o.injection_yield).toBe(o.yielded_keys / o.injected_keys);
    // 分母是"注入过的 key 去重全集"，它可以大于 injections（一次注入交付多条）
    expect(o.injected_keys).toBeGreaterThanOrEqual(0);
  });

  it("empty_ratio 乘 injections 必须是整数次（比率背后是计数，不许随手写分数）", () => {
    const o = VIEW.overview;
    const emptyInjections = o.store_empty_ratio! * o.injections;
    expect(Number.isInteger(emptyInjections)).toBe(true);
    expect(emptyInjections).toBeLessThanOrEqual(o.injections);
  });

  it("零注入那档用 null 而不是 0（「没注入过」与「注入了但都空」是两个结论）", () => {
    expect(VIEW_EMPTY.overview.store_empty_ratio).toBeNull();
    expect(VIEW_EMPTY.overview.injection_yield).toBeNull();
    // 阳性对照：有账的那一档确实给了数，否则上面两个 null 是空集里的真话
    expect(VIEW.overview.store_empty_ratio).not.toBeNull();
  });
});
