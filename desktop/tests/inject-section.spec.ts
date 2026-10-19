// Q11 第三条：组件层——`InjectSection.vue`（注入侧观测只读区块）。
// 为什么挑它：它同时踩在三件基建上（IPC 替身 / i18n 双档 / 状态呈现），且**没有**任何
// 交互负担（只读 + 手动刷新），是"组件测试运行面是否真能跑"的最小充分样本。
//
// ⚠ **断言一律用 key 而不是中文串**（用户 2026-10-03 裁定「GUI 全量 i18n 中英可切、默认中文」
// + 前端标准「同类只许一种形态」）：把中文文案抄进断言 = 测试替界面锁死一种语言，
// 而它恰恰证明不了双语那一半。这里改为「渲染文本必须等于当前档下该 key 的解析值」，
// 再**同一条用例切到 en 跑一遍**——两档的 title 若相同，说明语言包有一档没填、
// 界面在静默回落，那正是这条用例要抓的东西。
//
// ⚠ S3 刀③b 之前这里有三条：有回执 / 没有可解释的 / 回执读不出来（三态各一条文案）
// + 四档占比上屏。那三态的数据源 = 学习档写的四信号回执 ⇒ 整族退役后只剩两态
// （还没拉到 / 有数），"库里没账"改由 overview 自己的 `null` 读数表达（见最后一条）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import InjectSection from "../src/components/memory/InjectSection.vue";
import { i18n, applyLocalePref } from "../src/i18n";
import { VIEW, VIEW_EMPTY } from "./inject-fixture";
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

describe("InjectSection（注入观测只读区块）的读数与双档", () => {
  it("区块头上屏，且 L0 三个读数按当前语言档渲染", async () => {
    onCommand("memory_inject_view", VIEW);
    const wrapper = mount(InjectSection, { global: { plugins: [i18n] } });
    await flushPromises();
    const text = wrapper.text();
    expect(text).toContain(t("memory.injectTitle"));
    // 五个数逐个查：整句文案改了分隔符不算红，读数丢一个才算红
    for (const n of [
      VIEW.overview.injections,
      VIEW.overview.injected_keys,
      VIEW.overview.yielded_keys,
    ]) {
      expect(text).toContain(String(n));
    }
    expect(text).toContain("20%"); // 空块率 0.2
    expect(text).toContain("80%"); // 注入产出率 0.8
    wrapper.unmount();
  });

  it("零注入的两个比率渲染成「—」而不是 0%（不许把「没注入过」演成「注入了但没用上」）", async () => {
    onCommand("memory_inject_view", VIEW_EMPTY);
    const wrapper = mount(InjectSection, { global: { plugins: [i18n] } });
    await flushPromises();
    const text = wrapper.text();
    expect(text).toContain(t("memory.injectUnknown"));
    expect(text).not.toContain("0%");
    wrapper.unmount();
  });

  it("同一份数据切到 en 后界面真的换词（中英可切不是开关存在，而是文案变了）", async () => {
    onCommand("memory_inject_view", VIEW);
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
