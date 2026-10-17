// Q14 第四刀：字号基座的解析与写入（`src/lib/ui-font.ts`）。
//
// 这一簇盯的是两件事，每件都对应一个"没有它就只能靠人盯"的失败模式：
//  ① 值域外**回落缺省**而不是钳到边界——钳与回落都给出一个合法 px，只有跟端点比
//    才分得开，所以负面断言（`not.toBe(UI_FONT_MIN)`）与正面断言同在此处（历批口径：
//    一条用例要抓的是别人抓不到的那一档）；
//  ② 非法输入不许演成一次生效：`applyUiFontPref(99)` 之后 DOM 上仍是 14px。
//
// ⚠ **第三件事不在这里**：后端常量 / 前端常量 / `style.css` 首帧字面值是同一声明的
// 三个投影，"三处必须同数"这条跨文件对账住在 `scripts/check-single-source.mjs`（SS2/SS2b 两档），
// 与本仓历批的 Rust↔TS 字段镜像判据同一落点。分工理由有两条：形状对账要按绝对路径读盘
// （happy-dom 环境里 `import.meta.url` 不是 `file:` scheme，测试内读同级文件会
// `TypeError: The URL must be of scheme file` —— 本批第一跑实测到的），而对账闸本来就
// 在必跑清单里，判据不必两个载体各写一遍。
import { afterEach, describe, expect, it } from "vitest";

import {
  UI_FONT_DEFAULT,
  UI_FONT_MAX,
  UI_FONT_MIN,
  applyUiFontPref,
  effectiveUiFontPx,
} from "../src/lib/ui-font";

const CSS_VAR = "--lw-ui-font-size";

afterEach(() => {
  document.documentElement.style.removeProperty(CSS_VAR);
});

describe("effectiveUiFontPx：值域内原样、值域外回落缺省", () => {
  it.each([UI_FONT_MIN, 13, UI_FONT_DEFAULT, 17, UI_FONT_MAX])(
    "%ipx 在值域内，原样返回",
    (px) => {
      expect(effectiveUiFontPx(px)).toBe(px);
    },
  );

  it("端点外回落到缺省，且**不是**钳到最近端点", () => {
    // 下限外：钳会给出 12，回落给出 14 ⇒ 只有跟端点比才分得开这两种实现
    expect(effectiveUiFontPx(UI_FONT_MIN - 1)).toBe(UI_FONT_DEFAULT);
    expect(effectiveUiFontPx(UI_FONT_MIN - 1)).not.toBe(UI_FONT_MIN);
    expect(effectiveUiFontPx(UI_FONT_MAX + 1)).toBe(UI_FONT_DEFAULT);
    expect(effectiveUiFontPx(UI_FONT_MAX + 1)).not.toBe(UI_FONT_MAX);
    expect(effectiveUiFontPx(0)).toBe(UI_FONT_DEFAULT);
    expect(effectiveUiFontPx(99)).toBe(UI_FONT_DEFAULT);
  });

  it("非整数与 NaN 也回落（后端 u32 之外不存在合法值，前端不替它凑）", () => {
    for (const bad of [13.5, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(effectiveUiFontPx(bad)).toBe(UI_FONT_DEFAULT);
    }
  });

  it("值域常量自己也要自证：缺省落在区间内、区间两端各差 2", () => {
    // 这条盯着"把 MIN 写成 18 / MAX 写成 12"这类翻转：区间一旦空集，上面所有
    // 合法值都会悄悄回落缺省，而界面看起来一切正常。
    expect(UI_FONT_MIN).toBeLessThan(UI_FONT_MAX);
    expect(effectiveUiFontPx(UI_FONT_DEFAULT)).toBe(UI_FONT_DEFAULT);
  });
});

describe("applyUiFontPref：写 <html> 上的那一个变量", () => {
  it("合法值直接落 DOM", () => {
    applyUiFontPref(16);
    expect(document.documentElement.style.getPropertyValue(CSS_VAR)).toBe("16px");
  });

  it("非法值不许演成一次生效：DOM 上仍是缺省", () => {
    applyUiFontPref(11);
    expect(document.documentElement.style.getPropertyValue(CSS_VAR)).toBe(
      `${UI_FONT_DEFAULT}px`,
    );
  });

  it("写入点是 inline 而不是 data 属性（inline 优先级高于 :root 的兜底值，覆盖才成立）", () => {
    applyUiFontPref(18);
    expect(document.documentElement.getAttribute("style")).toContain(CSS_VAR);
  });
});
