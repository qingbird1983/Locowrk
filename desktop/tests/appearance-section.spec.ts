// S4 刀④：外观分区的字号行（`components/settings/AppearanceSection.vue`）。
//
// 字号从「12–18 数字框」收成小/中/大三档（决策人 2026-10-20：用户视角只留
// 预置，不填数值猜最优）。断言一律用 **i18n key 的当前档解析值**而不是中文串
// （同 `tree-board.spec.ts` 那条纪律），关键条目再同一条用例切到 en 跑一遍。
//
// 这一簇的四条各抓一层，彼此不重叠：
//  ① 三档按 lib/ui-font.ts 的预置顺序渲染、行名与提示在场 ⇒ 抓"模板里另抄一份
//     档值/文案"；
//  ② 选中项来自**投影**而不是本地缺省 ⇒ 抓"打开设置页时选中的档与生效值不一致"；
//  ③ 点选发一趟 `settings_set` 且 payload 只有 `appearance.uiFontSize` ⇒ 抓
//     "点了不变/多发/把整份 appearance 抄进 payload"；
//  ④ 存量非档值（旧手填 13）就近显示「标准」且**不写盘** ⇒ 抓"打开设置页就把
//     用户的存量值悄悄改掉"。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

import AppearanceSection from "../src/components/settings/AppearanceSection.vue";
import { i18n, applyLocalePref } from "../src/i18n";
import { UI_FONT_DEFAULT, UI_FONT_MAX, UI_FONT_MIN } from "../src/lib/ui-font";
import { useSettingsStore } from "../src/stores/settings";
import type { SettingsView } from "../src/types";
import { invokeCalls, onCommand, resetIpc } from "./tauri-stub";

type Appearance = SettingsView["appearance"];

const t = (key: string, params?: Record<string, unknown>) =>
  String(i18n.global.t(key, params as never));

const withGlobal = () => ({ global: { plugins: [i18n] } });

const viewWith = (appearance: Appearance) =>
  ({ locale: "zh-CN", appearance }) as SettingsView;

/** 字号那一个 radiogroup：按无障碍名取，不按"第二个分组"取——主题行也在同一
 *  分区里，位置断言会在行序调整时静默换靶。 */
function fontGroup(wrapper: ReturnType<typeof mount>) {
  const label = t("settings.fontSizeTitle");
  const group = wrapper
    .findAll('[role="radiogroup"]')
    .find((g) => g.attributes("aria-label") === label);
  if (!group) throw new Error(`font radiogroup not found: ${label}`);
  return group;
}

function fontRadios(wrapper: ReturnType<typeof mount>) {
  return fontGroup(wrapper).findAll('[role="radio"]');
}

/** 当前选中档的文案（未选中 = undefined，也把"一个都没选/选了俩"暴露出来）。 */
function checkedLabel(wrapper: ReturnType<typeof mount>) {
  const checked = fontRadios(wrapper).filter(
    (r) => r.attributes("aria-checked") === "true",
  );
  return checked.length === 1 ? checked[0]?.text() : undefined;
}

function seed(uiFontSize: number) {
  const settings = useSettingsStore();
  settings.view = viewWith({ theme: "light", uiFontSize });
  return settings;
}

beforeEach(() => {
  setActivePinia(createPinia());
  applyLocalePref("zh-CN");
  resetIpc();
});

describe("外观分区的字号行", () => {
  it("三档按预置顺序渲染，行名与提示在场；切 en 后旧中文不在场", () => {
    seed(UI_FONT_DEFAULT);
    const wrapper = mount(AppearanceSection, withGlobal());
    const text = wrapper.text();
    expect(text).toContain(t("settings.fontSizeTitle"));
    expect(text).toContain(t("settings.fontSizeHint"));
    // 档序与文案：值域与顺序的真源是 lib/ui-font.ts，模板不许另抄一份。
    expect(fontRadios(wrapper).map((r) => r.text())).toEqual([
      t("settings.fontSizeSmall"),
      t("settings.fontSizeMedium"),
      t("settings.fontSizeLarge"),
    ]);
    // 双语同一条：记下 zh 档的行名再切 en——断言里不出现中文字面量（q11 S5 那条判据），
    // 而"切档后旧的中文串不在场"这一半仍然被证明，因为比较对象就是它自己。
    const zhTitle = t("settings.fontSizeTitle");
    applyLocalePref("en");
    const en = mount(AppearanceSection, withGlobal());
    expect(en.text()).toContain(t("settings.fontSizeTitle"));
    expect(en.text()).not.toContain(zhTitle);
    expect(fontRadios(en).map((r) => r.text())).toEqual([
      t("settings.fontSizeSmall"),
      t("settings.fontSizeMedium"),
      t("settings.fontSizeLarge"),
    ]);
  });

  it("选中项 = 投影里的值，不是本地缺省", () => {
    seed(UI_FONT_MAX);
    const wrapper = mount(AppearanceSection, withGlobal());
    // 缺省是 14（标准）；投影 18 ⇒ 必须是「大」——否则这条会红。
    expect(checkedLabel(wrapper)).toBe(t("settings.fontSizeLarge"));
  });

  it("点选「大」发一趟 settings_set，payload 只带 appearance.uiFontSize", async () => {
    seed(UI_FONT_MIN);
    onCommand("settings_set", viewWith({ theme: "light", uiFontSize: UI_FONT_MAX }));
    const wrapper = mount(AppearanceSection, withGlobal());
    const large = fontRadios(wrapper).find(
      (r) => r.text() === t("settings.fontSizeLarge"),
    );
    if (!large) throw new Error("large option not found");
    await large.trigger("click");
    await flushPromises();
    const calls = invokeCalls().filter((c) => c.cmd === "settings_set");
    expect(calls).toHaveLength(1);
    expect((calls[0]?.args as Record<string, unknown>).appearance).toEqual({
      uiFontSize: UI_FONT_MAX,
    });
    // 提交后以返回投影为准：选中的是「大」
    expect(checkedLabel(wrapper)).toBe(t("settings.fontSizeLarge"));
  });

  it("存量非档值就近显示「标准」，且打开页面不发 IPC 改盘", async () => {
    seed(13); // 旧数字框时代手填的中间值
    const wrapper = mount(AppearanceSection, withGlobal());
    await flushPromises();
    expect(checkedLabel(wrapper)).toBe(t("settings.fontSizeMedium"));
    // 只是显示就近档，存量 13 要等用户点选才被替换
    expect(invokeCalls()).toHaveLength(0);
  });
});
