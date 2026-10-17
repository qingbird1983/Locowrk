// 台账 T16：三份「即时生效」通路（`setLocale` / `setTheme` / `setUiFontSize`）的
// 共同形状。本文件的判据不是"能不能改主题"，而是这三份**必须同构**这件事：
//
//   patch → 成功后按**返回投影**呈现 → 任一步失败按**进来之前的那份投影**回滚
//
// 三条读数分别钉三个洞：
//  ① 成功路径吃返回投影而不是入参（Q14 在字号面已有的那条判据，语言/主题两路此前没有；
//     实现若拿入参直接 apply，后端拒绝时界面已经变了而磁盘没变 = "看着生效了、重启回原样"）
//  ② IPC 失败：`error` 有条目 + 呈现按旧投影回滚 + `view` 引用没被换掉
//  ③ **apply 自己抛错**：这是本刀新抓的洞。改前三份都写 `catch { apply(this.view…) }`，
//     而如果 `setSettings` 已经成功、`this.view` 已被换成新投影，"回滚"就回到了**新**投影
//     = 等于没回滚。③ 在合一之前跑必红（红法见提交正文），合完之后绿。
//
// ⚠ 夹具只给 `locale` 与 `appearance` 两块（三个 action 只读这两块），其余字段缺席 ⇒
//   本簇用例一律不得读它们。三个 apply 替身化，是为了能断言"收到的到底是哪个值"，
//   也能制造"呈现层自己炸了"这一态（真 DOM 写不出这个异常）。
// ⚠ 投影一律比 `toEqual` 不比 `toBe`：Pinia 把 state 包成 reactive 代理，`s.view` 与
//   传进去那个对象**引用不相等**——第一版按 `toBe` 写，九条全红，红得跟实现有没有
//   缺陷毫无关系（判据自己错，不是代码错；记在这儿免得下一个人再踩）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Mock } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});
vi.mock("../src/i18n", () => ({ applyLocalePref: vi.fn() }));
vi.mock("../src/lib/theme", () => ({ applyThemePref: vi.fn() }));
vi.mock("../src/lib/ui-font", () => ({ applyUiFontPref: vi.fn() }));

import { createPinia, setActivePinia } from "pinia";

import { applyLocalePref } from "../src/i18n";
import { applyThemePref } from "../src/lib/theme";
import { applyUiFontPref } from "../src/lib/ui-font";
import { useSettingsStore } from "../src/stores/settings";
import type { SettingsView } from "../src/types";
import { onCommand, resetIpc } from "./tauri-stub";

type Store = ReturnType<typeof useSettingsStore>;

/** 进来之前的那份投影（回滚必须回到它）。 */
const OLD = {
  locale: "zh-CN",
  appearance: { theme: "dark", uiFontSize: 15 },
} as unknown as SettingsView;

/** 三档 case：入参**故意**与返回投影里的值不同（后端纠正时的真实形状）。 */
const CASES: {
  name: string;
  action: (s: Store) => Promise<void>;
  fresh: SettingsView;
  /** 返回投影里该被 apply 的那个值 */
  applied: string | number;
  /** 旧投影里该被 apply 回来的那个值 */
  rolledBack: string | number;
  spy: Mock;
}[] = [
  {
    name: "setLocale",
    action: (s) => s.setLocale("en"),
    fresh: { locale: "system", appearance: { theme: "dark", uiFontSize: 15 } } as unknown as SettingsView,
    applied: "system",
    rolledBack: "zh-CN",
    spy: vi.mocked(applyLocalePref),
  },
  {
    name: "setTheme",
    action: (s) => s.setTheme("light"),
    fresh: { locale: "zh-CN", appearance: { theme: "system", uiFontSize: 15 } } as unknown as SettingsView,
    applied: "system",
    rolledBack: "dark",
    spy: vi.mocked(applyThemePref),
  },
  {
    name: "setUiFontSize",
    action: (s) => s.setUiFontSize(17),
    fresh: { locale: "zh-CN", appearance: { theme: "dark", uiFontSize: 13 } } as unknown as SettingsView,
    applied: 13,
    rolledBack: 15,
    spy: vi.mocked(applyUiFontPref),
  },
];

beforeEach(() => {
  setActivePinia(createPinia());
  resetIpc();
  for (const c of CASES) c.spy.mockReset();
});

describe("即时生效类三份同构（T16）", () => {
  for (const c of CASES) {
    describe(c.name, () => {
      it("① 成功时呈现吃**返回投影**里的值，不吃入参", async () => {
        const s = useSettingsStore();
        s.view = OLD;
        onCommand("settings_set", c.fresh);
        await c.action(s);
        expect(s.error).toBe("");
        expect(c.spy).toHaveBeenCalledTimes(1);
        expect(c.spy).toHaveBeenCalledWith(c.applied);
        expect(s.view).toEqual(c.fresh);
      });

      it("② IPC 失败：error 有条目、呈现回旧投影、view 引用没被换掉", async () => {
        const s = useSettingsStore();
        s.view = OLD;
        // 不预置 settings_set ⇒ 替身 reject（"忘了喂数据"长不成通过）
        await c.action(s);
        expect(s.error).toContain("unstubbed command");
        expect(s.view).toEqual(OLD);
        expect(c.spy).toHaveBeenLastCalledWith(c.rolledBack);
      });

      it("③ 呈现层自己抛错：view 与呈现一起回到**进来的那份**投影", async () => {
        const s = useSettingsStore();
        s.view = OLD;
        onCommand("settings_set", c.fresh);
        c.spy.mockImplementationOnce(() => {
          throw new Error("呈现层炸了");
        });
        await c.action(s);
        expect(s.error).toContain("呈现层炸了");
        expect(s.view).toEqual(OLD);
        expect(c.spy).toHaveBeenLastCalledWith(c.rolledBack);
      });
    });
  }
});
