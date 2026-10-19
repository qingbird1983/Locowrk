// S2 刀③ + 刀④c：上下文分区那两枚压缩开关（`components/settings/ContextSection.vue`）。
//
// 形状照 `appearance-section.spec.ts`（同一种"挂一个分区、灌一份投影、点保存"的
// 取证载体），断言一律走 **i18n key 的当前档解析值**而不是中文串。
//
// 各条抓一层，彼此不重叠：
//  ① 开关初值来自**投影** ⇒ 抓"设置页显示开、后端其实关着"（打开面板看到的和实际生效的
//     必须是同一份，这是 v2.138 那条「看到的 = 触发用的」在开关维度上的同一条纪律）；
//  ② 翻开关**不自动**发 IPC，点保存才发一趟 ⇒ 抓"每点一下发一趟往返"（本区是显式保存制）；
//  ③ 保存载荷把**界面上真渲染过的键**全数带上 ⇒ 抓"新字段漏在后端边界上、界面永远关不掉
//     这一层"（漏带一个键时 `ContextUpdate` 走 serde default = 保持不变，于是按钮看着按下
//     去了、其实什么都没改）。反过来，本区**故意不渲染**的那两枚（摘要模型 / 自定义指令）
//     不许出现在载荷里——前者按"同类只许一种形态"跟着「记忆管理器」住进模型分区，后者要
//     等设置页多行输入这一新形态获批；替用户写回他没看过的值，是另一种骗；
//  ④ 区里三枚开关的 `aria-label` 互不相同 ⇒ 抓"label 抄漏 = 两个开关同名"（读屏无法区分，
//     而本文件的选择器也会立刻打空——这是"按标签取、不按类型捞第一个"那条口径）；
//  ⑤ 深度压缩单独可关 ⇒ 抓"两枚开关串在一起"（关了深度压缩不该把微压缩一起带走，
//     三层减速是三条独立的可关那一格，这正是可配置原则②在这里的落点）。
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

/** 上下文分区投影的十一键。缺省值**照后端 `ContextSettings::default` 抄，不照想象编**：
 *  滑杆 100_000 / 进度条开 / 预算 24 KiB / 微压缩开 / 深度压缩开 / 目标比 50 /
 *  近期原文额度 15_000 / 摘要模型空 / 自定义指令空 / 微压缩保留 5 组 / 滑窗保留 1 段。
 *  ⚠ 真源是后端 `settings/context.rs` 那几颗 `*_DEFAULT`；这里与 `ContextSection.vue`
 *  的 form 初值都是它的镜像，改了后端要同步这两处（同步漏掉的后果由下面那条"初值来自
 *  投影"用例挡一半——投影在场时镜像不参与判定）。 */
const ctx = (over: Partial<Context> = {}): Context => ({
  thresholdTokens: 100_000,
  usageBar: true,
  toolResultBudgetKib: 24,
  microEnabled: true,
  compactEnabled: true,
  compactTargetPct: 50,
  compactKeepTokens: 15_000,
  compactModel: "",
  compactInstructions: "",
  microKeepGroups: 5,
  slideKeepRuns: 1,
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

describe("上下文分区的压缩开关", () => {
  it("两枚压缩开关的行名与说明都渲染出来，切到 en 后同一 key 换成英文（中英成对的实跑面）", () => {
    seed();
    const wrapper = mount(ContextSection, withGlobal());
    const zhLabel = t("settings.microCompact");
    expect(wrapper.text()).toContain(zhLabel);
    expect(wrapper.text()).toContain(t("settings.microCompactHint"));
    expect(wrapper.text()).toContain(t("settings.deepCompact"));
    expect(wrapper.text()).toContain(t("settings.deepCompactHint"));

    applyLocalePref("en");
    const en = mount(ContextSection, withGlobal());
    expect(en.text()).toContain(t("settings.microCompact"));
    expect(en.text()).toContain(t("settings.deepCompact"));
    expect(en.text()).not.toContain(zhLabel);
    applyLocalePref("zh-CN");
  });

  it("两枚开关的初值都 = 投影里那两枚，不是本地缺省", () => {
    seed({ microEnabled: false, compactEnabled: false });
    const wrapper = mount(ContextSection, withGlobal());
    expect(switchByLabel(wrapper, t("settings.microCompact")).attributes("aria-checked")).toBe(
      "false",
    );
    expect(switchByLabel(wrapper, t("settings.deepCompact")).attributes("aria-checked")).toBe(
      "false",
    );
  });

  it("翻开关不发 IPC，点保存才发一趟，且载荷只含界面上渲染过的那九键", async () => {
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
      compactEnabled: true,
      compactTargetPct: 50,
      compactKeepTokens: 15_000,
      microKeepGroups: 5,
      slideKeepRuns: 1,
      // 断言用 toEqual（不是 objectContaining）：多一个键也算红。摘要模型与自定义指令
      // 两枚故意不在载荷里——本区没有它们的输入控件。
    });
  });

  it("区里三枚开关各有自己的标签（进度条 / 微压缩 / 深度压缩互不同名）", () => {
    seed();
    const wrapper = mount(ContextSection, withGlobal());
    const labels = wrapper
      .findAll('button[role="switch"]')
      .map((b) => b.attributes("aria-label") ?? "");
    expect(labels).toHaveLength(3);
    expect(new Set(labels).size).toBe(3);
    expect(labels).toContain(t("settings.usageBar"));
    expect(labels).toContain(t("settings.microCompact"));
    expect(labels).toContain(t("settings.deepCompact"));
  });

  it("深度压缩单独关得掉：载荷里 compactEnabled=false 而 microEnabled 保持原值", async () => {
    seed({ microEnabled: true, compactEnabled: true });
    onCommand("settings_set", viewWith(ctx({ compactEnabled: false })));
    const wrapper = mount(ContextSection, withGlobal());

    await switchByLabel(wrapper, t("settings.deepCompact")).trigger("click");
    await clickSave(wrapper);
    const calls = invokeCalls();
    const sent = calls[calls.length - 1]?.args?.context as Record<string, unknown>;
    expect(sent.compactEnabled).toBe(false);
    expect(sent.microEnabled, "两枚开关不许串在一起（三层减速各有关的那一格）").toBe(true);
  });

  // 刀⑥：四枚数值档。这一条抓的是"新字段漏在后端边界上"的另一半——**初值方向**：
  // 把投影灌成 7 而界面显示 form 里那份镜像缺省 5，用户看到的就是一个后端不认的数，
  // 与他刚保存的值差一格（v2.138「看到的 = 触发用的」在数值档上的同一条纪律）。
  it("四枚数值档的初值来自投影，改完照原值发回", async () => {
    seed({
      microKeepGroups: 7,
      slideKeepRuns: 3,
      compactTargetPct: 40,
      compactKeepTokens: 8_000,
    });
    const wrapper = mount(ContextSection, withGlobal());
    // `element` 的静态类型是 Element，取 value 要落到 HTMLInputElement 上（断言一次，
    // 下面四处就不必各写一遍 cast）
    const num = (id: string) => wrapper.get(`#${id}`).element as HTMLInputElement;
    expect(num("context-micro-keep-groups").value).toBe("7");
    expect(num("context-slide-keep-runs").value).toBe("3");
    expect(num("context-compact-target-pct").value).toBe("40");
    expect(num("context-compact-keep-tokens").value).toBe("8000");

    await wrapper.get("#context-slide-keep-runs").setValue("5");
    await clickSave(wrapper);
    const calls = invokeCalls();
    const sent = calls[calls.length - 1]?.args?.context as Record<string, unknown>;
    expect(sent.slideKeepRuns).toBe(5);
    expect(sent.microKeepGroups, "同一趟里没动的那枚要照投影原值发回，不许丢").toBe(7);
    expect(sent.compactTargetPct).toBe(40);
    expect(sent.compactKeepTokens).toBe(8_000);
  });
});
