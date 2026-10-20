// S3 刀④b：AC-S3-8 扣减链的**逐态**机械面。
//
// 真窗口那趟只撞得到"滑杆说话"与"旧快照"两态（当前模型窗口 1M，滑杆上限
// 800K，永远比窗口空位小）；"窗口未知"与"窗口封顶"靠这一格补，不能只写在
// 施工记录里。断言一律走 i18n key 的解析值 + `compactNumber` 现算，不锁中文串。
//
// 各条抓一层：
//  ① 旧快照（三字段缺省 0）⇒ 整块不渲染——前端不按当前设置替历史轮次现算；
//  ② 窗口未知 ⇒ 只出「有效阈值」一行，预留与空位都不摆（没有窗口就没有减法）；
//  ③ 滑杆说话 ⇒ 三行全出，空位 = 窗口 − 有效阈值（不是窗口 − 预留）；
//  ④ 窗口封顶 ⇒ 说明文字换成 roomWins（滑杆说了不算）；
//  ⑤ 来源三档 ⇒ 同一个数按"模型钉死 / 用户上限封顶 / 缺省"给出不同说法。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});
vi.mock("../src/lib/api", () => ({
  memoryTreeView: vi.fn(async () => null),
}));

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

import ContextUsagePopover from "../src/components/chat/ContextUsagePopover.vue";
import { i18n } from "../src/i18n";
import { compactNumber } from "../src/lib/format";

const t = (key: string, params?: Record<string, unknown>) =>
  String((params ? i18n.global.t(key, params) : i18n.global.t(key)) as string);

type Usage = {
  used: number;
  threshold: number;
  windowTokens: number;
  toolsEstTokens: number;
  sliderTokens: number;
  outputReserveTokens: number;
  outputReserveSource: string;
};

const withUsage = (over: Partial<Usage>) =>
  mount(ContextUsagePopover, {
    props: {
      usage: {
        used: 8_000,
        threshold: 100_000,
        windowTokens: 1_048_576,
        toolsEstTokens: 0,
        sliderTokens: 100_000,
        outputReserveTokens: 21_000,
        outputReserveSource: "capped",
        ...over,
      },
      readout: "used",
    },
    global: { plugins: [i18n] },
  });

/// 按行标签取那一行的数值格（不靠"文本里出现过这个数"——同一个数可能在
/// 阈值行里也出现，那样断言空位等于阈值也能绿）。
const rowValue = (w: ReturnType<typeof withUsage>, label: string) =>
  w
    .findAll(".kv")
    .find((r) => r.text().includes(label))
    ?.find(".kv__v")
    .text() ?? "";

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("context usage popover · 扣减链逐态", () => {
  it("旧快照（sliderTokens = 0）整块不渲染", () => {
    const w = withUsage({
      sliderTokens: 0,
      outputReserveTokens: 0,
      outputReserveSource: "",
    });
    const text = w.text();
    expect(text).not.toContain(t("usage.outputReserve"));
    expect(text).not.toContain(t("usage.effectiveThreshold"));
    expect(text).not.toContain(t("usage.headroom"));
  });

  it("窗口未知 ⇒ 只出有效阈值一行，阈值原样取滑杆", () => {
    const w = withUsage({ windowTokens: 0 });
    const text = w.text();
    expect(text).toContain(t("usage.effectiveThreshold"));
    expect(text).toContain(
      t("usage.chainNoWindow", { slider: compactNumber(100_000) }),
    );
    expect(text).not.toContain(t("usage.outputReserve"));
    expect(text).not.toContain(t("usage.headroom"));
  });

  it("滑杆小于窗口空位 ⇒ 阈值取滑杆，空位 = 窗口 − 有效阈值", () => {
    const w = withUsage({
      windowTokens: 200_000,
      threshold: 100_000,
      sliderTokens: 100_000,
      outputReserveTokens: 21_000,
    });
    const text = w.text();
    expect(text).toContain(t("usage.outputReserve"));
    expect(text).toContain(
      t("usage.chainSliderWins", {
        slider: compactNumber(100_000),
        room: compactNumber(200_000 - 21_000),
      }),
    );
    // 空位不是 179K（窗口 − 预留），而是 100K（窗口 − 有效阈值）。
    expect(rowValue(w, t("usage.headroom"))).toBe(
      `${compactNumber(100_000)} tokens`,
    );
    expect(text).toContain(t("usage.headroomHint"));
  });

  it("窗口封顶 ⇒ 说明文字换成 roomWins", () => {
    const w = withUsage({
      windowTokens: 200_000,
      threshold: 179_000,
      sliderTokens: 400_000,
      outputReserveTokens: 21_000,
    });
    expect(w.text()).toContain(
      t("usage.chainRoomWins", {
        slider: compactNumber(400_000),
        room: compactNumber(200_000 - 21_000),
      }),
    );
  });

  it("来源三档各有说法（同一个数 8K 不许混成一句）", () => {
    const base = {
      windowTokens: 200_000,
      threshold: 100_000,
      sliderTokens: 100_000,
      outputReserveTokens: 8_000,
    };
    expect(withUsage({ ...base, outputReserveSource: "model" }).text()).toContain(
      t("usage.reserveFromModel", { n: compactNumber(8_000) }),
    );
    expect(withUsage({ ...base, outputReserveSource: "capped" }).text()).toContain(
      t("usage.reserveCapped", { n: compactNumber(8_000) }),
    );
    expect(
      withUsage({ ...base, outputReserveSource: "default" }).text(),
    ).toContain(t("usage.reserveDefault", { n: compactNumber(8_000) }));
  });
});
