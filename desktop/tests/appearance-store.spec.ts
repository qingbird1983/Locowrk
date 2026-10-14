// Q14 第四刀：`stores/settings.ts` 的字号基座通路（即时生效类第三个动作）。
//
// 盯的是「**以后端投影为准**」这一条，而不是"发了正确的数"：
// setUiFontSize(16) 之后 DOM 上写的是**返回投影里的值**。若实现写成
// `applyUiFontPref(px)`（拿入参直接 apply），那么后端越界拒绝、或返回一份被别的
// 分区校验顺带纠正过的投影时，界面已经放大而磁盘没变 ——「看着生效了、重启回原样」
// 这类最难查的漂就诞生在这里。所以下面有一条故意让**入参与返回投影不一致**的用例。
//
// ⚠ 夹具只给 `appearance` 一块，其余字段缺席 ⇒ 本簇用例一律**不得**读它们
// （读到了就是 undefined，用例会直接红，不会静默通过）。`Appearance` 用索引类型
// 取，字段名与完整性由 vue-tsc 钉（`uiFontSz` 这种拼写错误编译不过）。
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { createPinia, setActivePinia } from "pinia";

import { UI_FONT_DEFAULT } from "../src/lib/ui-font";
import { useSettingsStore } from "../src/stores/settings";
import type { SettingsView } from "../src/types";
import { invokeCalls, onCommand, resetIpc } from "./tauri-stub";

type Appearance = SettingsView["appearance"];

const CSS_VAR = "--lw-ui-font-size";
const domFont = () => document.documentElement.style.getPropertyValue(CSS_VAR);

/** 只装 appearance（+ locale，init 会 apply 它）的最小投影：其余字段在本簇里不被读。 */
const viewWith = (appearance: Appearance) =>
  ({ locale: "zh-CN", appearance }) as SettingsView;

beforeEach(() => {
  setActivePinia(createPinia());
  resetIpc();
  document.documentElement.style.removeProperty(CSS_VAR);
});

// applyThemePref 会在 <html> 上挂 data-theme 且 system 档会留一个媒体查询监听：
// 用例之间必须清，否则「init 写了 dark」这条会给下一条留下非空读数。
afterEach(() => {
  delete document.documentElement.dataset.theme;
});

describe("setUiFontSize", () => {
  it("发的 patch 形状 = 只带 uiFontSize 的 appearance（theme 缺席 = 保持不变）", async () => {
    onCommand("settings_set", viewWith({ theme: "light", uiFontSize: 16 }));
    const settings = useSettingsStore();
    await settings.setUiFontSize(16);
    const calls = invokeCalls();
    expect(calls).toHaveLength(1);
    expect(calls[0]?.cmd).toBe("settings_set");
    expect((calls[0]?.args as Record<string, unknown>).appearance).toEqual({
      uiFontSize: 16,
    });
  });

  it("成功后 DOM 取**返回投影**，不取入参（后端拒绝时界面不许已经放大）", async () => {
    // 故意让入参 17、返回投影 13：实现若拿入参 apply，DOM 会是 17px ⇒ 本用例红
    onCommand("settings_set", viewWith({ theme: "light", uiFontSize: 13 }));
    const settings = useSettingsStore();
    await settings.setUiFontSize(17);
    expect(domFont()).toBe("13px");
    expect(settings.view?.appearance.uiFontSize).toBe(13);
  });

  it("IPC 失败时 error 有条目，且 DOM 按旧投影回滚（不是停在缺省、也不是停在想设的值）", async () => {
    const settings = useSettingsStore();
    settings.view = viewWith({ theme: "light", uiFontSize: 15 });
    // 先跑一趟成功的：让 DOM 与旧投影一致，才能证明后面那次"回滚"真的发生过
    // （DOM 一开始本来就是空的，空 → 空看不出回滚）。
    onCommand("settings_set", viewWith({ theme: "light", uiFontSize: 15 }));
    await settings.setUiFontSize(15);
    expect(domFont()).toBe("15px");
    // 第二趟：清空预置 ⇒ 替身 reject（未预置 = reject 而不是 resolve undefined，
    // 这条替身纪律让"忘了喂数据"的用例长不成通过）
    resetIpc();
    await settings.setUiFontSize(18);
    expect(settings.error).toContain("unstubbed command");
    expect(domFont()).toBe("15px");
  });

  it("越界值由 lib/ui-font 兜住：投影里出现 99 也不写进 DOM", async () => {
    onCommand("settings_set", viewWith({ theme: "light", uiFontSize: 99 }));
    const settings = useSettingsStore();
    await settings.setUiFontSize(99);
    expect(domFont()).toBe(`${UI_FONT_DEFAULT}px`);
  });
});

describe("init 应用三项呈现偏好", () => {
  it("启动时把投影里的字号基座写进 DOM（首帧兜底之后由这一句接管）", async () => {
    onCommand("settings_get", viewWith({ theme: "dark", uiFontSize: 12 }));
    const settings = useSettingsStore();
    await settings.init();
    expect(domFont()).toBe("12px");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("settings_get 失败不阻塞：error 有值且 DOM 未被写入（不是被写成缺省冒充成功）", async () => {
    const settings = useSettingsStore();
    await settings.init();
    expect(settings.error).toContain("unstubbed command");
    expect(domFont()).toBe("");
  });
});
