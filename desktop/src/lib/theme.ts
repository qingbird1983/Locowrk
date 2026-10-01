// 主题应用唯一执行者（UI shell and patterns.md §六）：settings.json
// appearance.theme → <html data-theme>。system 档跟随系统
// prefers-color-scheme 并监听变化；组件禁止自读媒体查询。
// 合法值与后端 settings/appearance.rs 的 THEMES 一一对应。

type ThemePref = "dark" | "light" | "system";
const PREFS: ThemePref[] = ["dark", "light", "system"];

let media: MediaQueryList | null = null;
let mediaHandler: ((e: MediaQueryListEvent) => void) | null = null;

function resolve(pref: ThemePref): "dark" | "light" {
  if (pref !== "system") return pref;
  return window.matchMedia("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}

/** 应用主题偏好（settings.init 与保存动作调用）；未知值回落 dark。 */
export function applyThemePref(theme: string): void {
  const pref: ThemePref = PREFS.includes(theme as ThemePref)
    ? (theme as ThemePref)
    : "dark";
  // 换档先摘旧监听（dark/light 档不挂监听）
  if (media && mediaHandler) {
    media.removeEventListener("change", mediaHandler);
    media = null;
    mediaHandler = null;
  }
  if (pref === "system") {
    media = window.matchMedia("(prefers-color-scheme: light)");
    mediaHandler = () => {
      document.documentElement.dataset.theme = resolve(pref);
    };
    media.addEventListener("change", mediaHandler);
  }
  document.documentElement.dataset.theme = resolve(pref);
}
