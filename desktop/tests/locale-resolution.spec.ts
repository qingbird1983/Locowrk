// Q11 第四条：语言解析（`src/i18n.ts::systemLocale/applyLocalePref`）。
// 它同时是**运行面的探针**：`systemLocale()` 读 `navigator.language`，没有 DOM 环境就跑不到
// 这一行——所以这三条断言绿，等于证明 happy-dom 真在提供 window/navigator，而不是
// "配置里写了环境但没人用它"。
// 口径出处 = DESIGN §13.5：`zh*` → zh-CN、`en*` → en、**无匹配回落中文**。
import { afterEach, describe, expect, it } from "vitest";
import { applyLocalePref, i18n, systemLocale } from "../src/i18n";

function setLang(value: string): void {
  Object.defineProperty(navigator, "language", { value, configurable: true });
}

const original = navigator.language;
afterEach(() => setLang(original));

describe("系统语言解析与偏好落档", () => {
  it("zh* 落中文档（含区域变体）", () => {
    setLang("zh-TW");
    expect(systemLocale()).toBe("zh-CN");
  });

  it("en* 落英文档，大小写不敏感", () => {
    setLang("EN-us");
    expect(systemLocale()).toBe("en");
  });

  it("无匹配语言回落 zh-CN，而不是回落成 key 本身（§13.5 明写「跟随系统、无匹配回落中文」）", () => {
    setLang("de-DE");
    expect(systemLocale()).toBe("zh-CN");
    setLang("");
    expect(systemLocale()).toBe("zh-CN");
  });

  it("pref = system 时按解析结果落档；显式 pref 直接生效", () => {
    setLang("en-GB");
    applyLocalePref("system");
    expect(i18n.global.locale.value).toBe("en");
    applyLocalePref("zh-CN");
    expect(i18n.global.locale.value).toBe("zh-CN");
  });
});
