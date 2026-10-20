// S4（specs/11 刀①）：上下文分区收敛成**三格**（`components/settings/ContextSection.vue`）。
//
// 形状照 `appearance-section.spec.ts`（同一种"挂一个分区、灌一份投影、点保存"的
// 取证载体），断言一律走 **i18n key 的当前档解析值**而不是中文串。
//
// 各条抓一层，彼此不重叠：
//  ① 三行都渲染，切到 en 后同一 key 换成英文（中英成对的实跑面）；
//  ② 两枚开关的初值来自**投影** ⇒ 抓"设置页显示开、后端其实关着"；
//  ③ 翻开关**不自动**发 IPC，点保存才发一趟 ⇒ 抓"每点一下发一趟往返"；
//  ④ 保存载荷**恰好三键** ⇒ 抓"新字段漏在后端边界上"与"没渲染的键替用户写回"
//     两种骗法（多一个键也算红）；
//  ⑤ 自动压缩单独关得掉：不许把用量条一起带走；
//  ⑥ 滑条初值来自投影、改档后照原值发回 ⇒ 抓"界面显示一个后端不认的数"。
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

/** 上下文分区投影的**三键**（真源 = 后端 `settings/context.rs`）。 */
const ctx = (over: Partial<Context> = {}): Context => ({
  thresholdTokens: 100_000,
  usageBar: true,
  compactEnabled: true,
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

describe("上下文分区的三格", () => {
  it("三行都渲染；切到 en 后同一 key 换成英文", () => {
    seed();
    const wrapper = mount(ContextSection, withGlobal());
    const zhLabel = t("settings.autoCompact");
    expect(wrapper.text()).toContain(t("settings.threshold"));
    expect(wrapper.text()).toContain(zhLabel);
    expect(wrapper.text()).toContain(t("settings.usageBar"));

    applyLocalePref("en");
    const en = mount(ContextSection, withGlobal());
    expect(en.text()).toContain(t("settings.autoCompact"));
    expect(en.text()).toContain(t("settings.usageBar"));
    expect(en.text()).not.toContain(zhLabel);
    applyLocalePref("zh-CN");
  });

  it("两枚开关的初值 = 投影里那两枚，不是本地缺省", () => {
    seed({ compactEnabled: false, usageBar: false });
    const wrapper = mount(ContextSection, withGlobal());
    expect(switchByLabel(wrapper, t("settings.autoCompact")).attributes("aria-checked")).toBe(
      "false",
    );
    expect(switchByLabel(wrapper, t("settings.usageBar")).attributes("aria-checked")).toBe(
      "false",
    );
  });

  it("翻开关不发 IPC，点保存才发一趟，且载荷恰为三键", async () => {
    seed({ compactEnabled: false });
    onCommand("settings_set", viewWith(ctx({ compactEnabled: true })));
    const wrapper = mount(ContextSection, withGlobal());
    const sw = switchByLabel(wrapper, t("settings.autoCompact"));

    await sw.trigger("click");
    expect(sw.attributes("aria-checked")).toBe("true");
    expect(invokeCalls(), "开关是草稿，不该一点就发往返").toHaveLength(0);

    await clickSave(wrapper);
    const calls = invokeCalls();
    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe("settings_set");
    // toEqual（不是 objectContaining）：多一个键也算红。
    expect(calls[0]?.args?.context).toEqual({
      thresholdTokens: 100_000,
      usageBar: true,
      compactEnabled: true,
    });
  });

  it("两枚开关各有自己的标签（自动压缩 / 用量条互不同名）", () => {
    seed();
    const wrapper = mount(ContextSection, withGlobal());
    const labels = wrapper
      .findAll('button[role="switch"]')
      .map((b) => b.attributes("aria-label") ?? "");
    expect(labels).toHaveLength(2);
    expect(new Set(labels).size).toBe(2);
    expect(labels).toContain(t("settings.autoCompact"));
    expect(labels).toContain(t("settings.usageBar"));
  });

  it("自动压缩单独关得掉：载荷里 compactEnabled=false 而 usageBar 保持原值", async () => {
    seed({ compactEnabled: true, usageBar: true });
    onCommand("settings_set", viewWith(ctx({ compactEnabled: false })));
    const wrapper = mount(ContextSection, withGlobal());

    await switchByLabel(wrapper, t("settings.autoCompact")).trigger("click");
    await clickSave(wrapper);
    const calls = invokeCalls();
    const sent = calls[calls.length - 1]?.args?.context as Record<string, unknown>;
    expect(sent.compactEnabled).toBe(false);
    expect(sent.usageBar, "两枚开关不许串在一起").toBe(true);
  });

  it("滑条初值来自投影，改档后照原值发回", async () => {
    seed({ thresholdTokens: 300_000 });
    const wrapper = mount(ContextSection, withGlobal());
    const slider = wrapper.get('input[type="range"]').element as HTMLInputElement;
    expect(slider.value).toBe("300000");

    await wrapper.get('input[type="range"]').setValue("400000");
    await clickSave(wrapper);
    const calls = invokeCalls();
    const sent = calls[calls.length - 1]?.args?.context as Record<string, unknown>;
    expect(sent.thresholdTokens).toBe(400_000);
    expect(sent.compactEnabled, "同一趟里没动的那枚要照投影原值发回").toBe(true);
  });

  it("本区没有数值输入框（普通用户只做选择，不填数）", () => {
    seed();
    const wrapper = mount(ContextSection, withGlobal());
    expect(wrapper.findAll('input[type="number"]')).toHaveLength(0);
    expect(wrapper.findAll('input[type="range"]')).toHaveLength(1);
  });
});
