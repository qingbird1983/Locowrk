// Q14 第四刀：外观分区的字号行（`components/settings/AppearanceSection.vue`）。
//
// 断言一律用 **i18n key 的当前档解析值**而不是中文串（同 `tree-board.spec.ts` 那条
// 纪律），关键条目再同一条用例切到 en 跑一遍。
//
// 这一簇的四条各抓一层，彼此不重叠：
//  ① 输入框初值来自**投影**而不是本地缺省 ⇒ 抓"打开设置页时数字与实际生效值不一致"；
//  ② change 提交一次且 payload 是 `{ uiFontSize }` ⇒ 抓"每敲一个字符发一趟 IPC"；
//  ③ 越界不发 IPC 且弹回当前生效值 ⇒ 抓"把空白格提交成 0"（v-model.number 对空串给 0）；
//  ④ min/max 绑的是常量 ⇒ 抓"值域被抄进模板变成第四出处"。
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

/** 字号输入框：按 control-id 关联的 id 取，不按 type=number 取（外观区里数字框只有
 *  这一个，但"按类型捞第一个"这类断言在下一行数值项进来时会静默换靶）。 */
function fontInput(wrapper: ReturnType<typeof mount>) {
  return wrapper.get("#appearance-font-size-input");
}

/** 模拟"在框里敲完再移开焦点"这条真实路径。
 * ⚠ **不用 `setValue`**：它除了 `input` 还会附带触发一次 `change`（本批第一跑就是
 * 在这里看到"越界那趟也发了一次 IPC"——其实是第二次 change 拿已弹回的合法值提交，
 * 而用户只有一次 blur）。直接写 `el.value` + `trigger("input")` 才与浏览器分工一致：
 * input 更新草稿、change 才提交。 */
async function typeInto(input: ReturnType<ReturnType<typeof mount>["get"]>, value: string) {
  (input.element as HTMLInputElement).value = value;
  await input.trigger("input");
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
  it("行名与提示渲染出来，提示里的值域走插值（文案不裸抄 12/18）", () => {
    seed(UI_FONT_DEFAULT);
    const wrapper = mount(AppearanceSection, withGlobal());
    const text = wrapper.text();
    expect(text).toContain(t("settings.fontSizeTitle"));
    expect(text).toContain(t("settings.fontSizeHint", { min: UI_FONT_MIN, max: UI_FONT_MAX }));
    // 双语同一条：记下 zh 档的行名再切 en——断言里不出现中文字面量（q11 S5 那条判据），
    // 而"切档后旧的中文串不在场"这一半仍然被证明，因为比较对象就是它自己。
    const zhTitle = t("settings.fontSizeTitle");
    applyLocalePref("en");
    const en = mount(AppearanceSection, withGlobal());
    expect(en.text()).toContain(t("settings.fontSizeTitle"));
    expect(en.text()).not.toContain(zhTitle);
  });

  it("输入框初值 = 投影里的值，不是本地缺省", () => {
    seed(16);
    const wrapper = mount(AppearanceSection, withGlobal());
    expect((fontInput(wrapper).element as HTMLInputElement).value).toBe("16");
  });

  it("change 提交一趟 IPC，payload 只带 uiFontSize；输入过程中不发", async () => {
    seed(14);
    onCommand("settings_set", viewWith({ theme: "light", uiFontSize: 16 }));
    const wrapper = mount(AppearanceSection, withGlobal());
    const input = fontInput(wrapper);
    await typeInto(input, "16");
    // input 事件只更新草稿，**不该**有调用（每敲一个字符发一趟往返）
    expect(invokeCalls()).toHaveLength(0);
    await input.trigger("change");
    const calls = invokeCalls();
    expect(calls).toHaveLength(1);
    expect((calls[0]?.args as Record<string, unknown>).appearance).toEqual({ uiFontSize: 16 });
    await flushPromises();
    // 提交后框里 = 后端返回的投影，不是草稿残留
    expect((input.element as HTMLInputElement).value).toBe("16");
  });

  it("越界与清空都不提交，输入框弹回当前生效值", async () => {
    seed(15);
    const wrapper = mount(AppearanceSection, withGlobal());
    const input = fontInput(wrapper);
    // 99：后端会拒，但这里连往返都不该发（红条说 12–18、用户看着空白格更难懂）
    await typeInto(input, "99");
    await input.trigger("change");
    expect(invokeCalls()).toHaveLength(0);
    expect((input.element as HTMLInputElement).value).toBe("15");
    // 空串：v-model.number 给 ""，Number("") = 0 —— 0 是合法 u32，一旦被提交
    // 后端会当成"越界"拒掉，但界面已把用户的清空动作演成一次失败保存
    await typeInto(input, "");
    await input.trigger("change");
    expect(invokeCalls()).toHaveLength(0);
    expect((input.element as HTMLInputElement).value).toBe("15");
  });

  it("min/max 绑常量：模板里没有裸数字值域，且端点与 ui-font 一致", () => {
    seed(UI_FONT_DEFAULT);
    const wrapper = mount(AppearanceSection, withGlobal());
    const el = fontInput(wrapper).element as HTMLInputElement;
    expect(el.min).toBe(String(UI_FONT_MIN));
    expect(el.max).toBe(String(UI_FONT_MAX));
  });
});
