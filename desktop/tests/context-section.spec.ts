// S2 刀③：上下文分区那枚「微压缩」开关（`components/settings/ContextSection.vue`）。
//
// 形状照 `appearance-section.spec.ts`（同一种"挂一个分区、灌一份投影、点保存"的
// 取证载体），断言一律走 **i18n key 的当前档解析值**而不是中文串。
//
// 四条各抓一层，彼此不重叠：
//  ① 开关初值来自**投影** ⇒ 抓"设置页显示开、后端其实关着"（打开面板看到的和实际生效的
//     必须是同一份，这是 v2.138 那条「看到的 = 触发用的」在开关维度上的同一条纪律）；
//  ② 翻开关**不自动**发 IPC，点保存才发一趟 ⇒ 抓"每点一下发一趟往返"（本区是显式保存制）；
//  ③ 保存载荷四个键全数带上，`microEnabled` 是翻过的那个值 ⇒ 抓"新字段漏在后端边界上、
//     界面永远关不掉这一层"（漏带一个键时 `ContextUpdate` 走 serde default = 保持不变，
//     于是按钮看着按下去了、其实什么都没改）；
//  ④ 区里两枚开关的 `aria-label` 互不相同 ⇒ 抓"label 抄漏 = 两个开关同名"（读屏无法区分，
//     而本文件的选择器也会立刻打空——这是那条"按标签取、不按类型捞第一个"的口径）。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

import ContextSection from "../src/components/settings/ContextSection.vue";
import { i18n, applyLocalePref } from "../src/i18n";
import { useSettingsStore } from "../src/stores/settings";
import type { SettingsView } from "../src/types";
import { invokeCalls, onCommand, resetIpc } from "./tauri-stub";

type Context = SettingsView["context"];

const t = (key: string) => String(i18n.global.t(key) as string);
const withGlobal = () => ({ global: { plugins: [i18n] } });

const viewWith = (context: Context) => ({ locale: "zh-CN", context }) as SettingsView;

/** 上下文分区的四键（缺省值照后端 `ContextSettings::default`，**照生产抄不照想象编**：
 *  滑杆 100_000 / 进度条开 / 预算 24 KiB / 微压缩开）。 */
const ctx = (over: Partial<Context> = {}): Context => ({
  thresholdTokens: 100_000,
  usageBar: true,
  toolResultBudgetKib: 24,
  microEnabled: true,
  ...over,
});

function switchByLabel(wrapper: ReturnType<typeof mount>, label: string) {
  // 标签用 JSON.stringify 包成选择器里的字符串字面量（值里有中文/空格也不用手工转义）
  return wrapper.get(`button[role="switch"][aria-label=${JSON.stringify(label)}]`);
}

function seed(over: Partial<Context> = {}) {
  const settings = useSettingsStore();
  settings.view = viewWith(ctx(over));
  return settings;
}

async function clickSave(wrapper: ReturnType<typeof mount>) {
  const btn = wrapper.findAll("button").find((b) => b.text() === t("settings.save"));
  expect(btn, "保存行得在这个分区里（本区是显式保存制）").toBeTruthy();
  await btn!.trigger("click");
  await flushPromises();
}

beforeEach(() => {
  setActivePinia(createPinia());
  applyLocalePref("zh-CN");
  resetIpc();
});

describe("上下文分区的微压缩开关", () => {
  it("行名与说明渲染出来，且切到 en 后同一 key 换成英文（中英成对的实跑面）", () => {
    seed();
    const wrapper = mount(ContextSection, withGlobal());
    const zhLabel = t("settings.microCompact");
    expect(wrapper.text()).toContain(zhLabel);
    expect(wrapper.text()).toContain(t("settings.microCompactHint"));

    applyLocalePref("en");
    const en = mount(ContextSection, withGlobal());
    expect(en.text()).toContain(t("settings.microCompact"));
    expect(en.text()).not.toContain(zhLabel);
    applyLocalePref("zh-CN");
  });

  it("开关初值 = 投影里的 microEnabled，不是本地缺省", () => {
    seed({ microEnabled: false });
    const wrapper = mount(ContextSection, withGlobal());
    expect(switchByLabel(wrapper, t("settings.microCompact")).attributes("aria-checked")).toBe(
      "false",
    );
  });

  it("翻开关不发 IPC，点保存才发一趟，且载荷四个键齐全", async () => {
    seed({ microEnabled: false });
    onCommand("settings_set", viewWith(ctx({ microEnabled: true })));
    const wrapper = mount(ContextSection, withGlobal());
    const sw = switchByLabel(wrapper, t("settings.microCompact"));

    await sw.trigger("click");
    expect(sw.attributes("aria-checked")).toBe("true");
    expect(invokeCalls(), "开关是草稿，不该一点就发往返").toHaveLength(0);

    await clickSave(wrapper);
    const calls = invokeCalls();
    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe("settings_set");
    expect(calls[0]?.args?.context).toEqual({
      thresholdTokens: 100_000,
      usageBar: true,
      toolResultBudgetKib: 24,
      microEnabled: true,
    });
  });

  it("区里两枚开关各有自己的标签（进度条 / 微压缩不同名）", () => {
    seed();
    const wrapper = mount(ContextSection, withGlobal());
    const labels = wrapper
      .findAll('button[role="switch"]')
      .map((b) => b.attributes("aria-label") ?? "");
    expect(labels).toHaveLength(2);
    expect(new Set(labels).size).toBe(2);
    expect(labels).toContain(t("settings.usageBar"));
    expect(labels).toContain(t("settings.microCompact"));
  });
});
