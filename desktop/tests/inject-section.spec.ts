// Q11 第三条：组件层——现例 = Q10 刚落的只读解释区块 `InjectSection.vue`。
// 为什么挑它：它同时踩在三件基建上（IPC 替身 / i18n 双档 / 三态呈现），且**没有**任何
// 交互负担（只读 + 手动刷新），是"组件测试运行面是否真能跑"的最小充分样本。
//
// ⚠ **断言一律用 key 而不是中文串**（用户 2026-10-03 裁定「GUI 全量 i18n 中英可切、默认中文」
// + 前端标准「同类只许一种形态」）：把中文文案抄进断言 = 测试替界面锁死一种语言，
// 而它恰恰证明不了双语那一半。这里改为「渲染文本必须等于当前档下该 key 的解析值」，
// 再**同一条用例切到 en 跑一遍**——两档的 title 若相同，说明语言包有一档没填、
// 界面在静默回落，那正是这条用例要抓的东西。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import InjectSection from "../src/components/memory/InjectSection.vue";
import { i18n, applyLocalePref } from "../src/i18n";
import { VIEW_NOT_PRESENT, VIEW_UNREADABLE, VIEW_WITH_ROW } from "./inject-fixture";
import { onCommand, resetIpc } from "./tauri-stub";

const t = (key: string) => String(i18n.global.t(key));
const zhThen = <T,>(run: () => T): T => {
  applyLocalePref("zh-CN");
  return run();
};

beforeEach(() => {
  resetIpc();
  applyLocalePref("zh-CN");
});

describe("InjectSection（只读解释区块）的三态与双档", () => {
  it("有回执那一档：区块头、行 key 与四档占比都上屏", async () => {
    onCommand("memory_inject_view", VIEW_WITH_ROW);
    const wrapper = mount(InjectSection, { global: { plugins: [i18n] } });
    await flushPromises();
    const text = wrapper.text();
    expect(text).toContain(t("memory.injectTitle"));
    expect(text).toContain("user-role"); // 行 key 是人查东西的把手，必须可见
    expect(text).toContain(String(VIEW_WITH_ROW.explain.rows[0]!.total));
    wrapper.unmount();
  });

  it("「还没有可解释的」与「读不出来」是两条不同文案，不共用（§二⑩ + UI shell 三态纪律）", async () => {
    onCommand("memory_inject_view", VIEW_NOT_PRESENT);
    const empty = mount(InjectSection, { global: { plugins: [i18n] } });
    await flushPromises();
    expect(empty.text()).toContain(t("memory.injectEmpty"));
    expect(empty.text()).not.toContain(t("memory.injectUnreadable"));
    empty.unmount();

    onCommand("memory_inject_view", VIEW_UNREADABLE);
    const broken = mount(InjectSection, { global: { plugins: [i18n] } });
    await flushPromises();
    expect(broken.text()).toContain(t("memory.injectUnreadable"));
    expect(broken.text()).not.toContain(t("memory.injectEmpty"));
    broken.unmount();
  });

  it("同一份数据切到 en 后界面真的换词（中英可切不是开关存在，而是文案变了）", async () => {
    onCommand("memory_inject_view", VIEW_WITH_ROW);
    const wrapper = mount(InjectSection, { global: { plugins: [i18n] } });
    await flushPromises();

    const zhTitle = zhThen(() => t("memory.injectTitle"));
    applyLocalePref("en");
    await wrapper.vm.$nextTick();
    const enTitle = t("memory.injectTitle");

    expect(enTitle).not.toBe(zhTitle); // 同字 = en 档没填，界面在回落
    expect(wrapper.text()).toContain(enTitle);
    expect(wrapper.text()).not.toContain(zhTitle);
    applyLocalePref("zh-CN");
    wrapper.unmount();
  });
});
