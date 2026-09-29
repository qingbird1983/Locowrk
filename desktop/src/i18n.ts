// vue-i18n 装配（DESIGN.md §13.5）：文案集中在 locales/，组件不硬编码。
// 当前语言缺省跟随系统语言、无匹配回落中文；持久化在数据根 settings.json
// （settings.rs），启动时由 settings store 应用已存偏好。
import { createI18n } from "vue-i18n";
import en from "./locales/en";
import zhCN from "./locales/zh-CN";

/** 系统语言解析：zh* → zh-CN，en* → en，无匹配回落中文。 */
export function systemLocale(): string {
  const lang = (navigator.language ?? "").toLowerCase();
  if (lang.startsWith("zh")) return "zh-CN";
  if (lang.startsWith("en")) return "en";
  return "zh-CN"; // 无匹配回落中文（§13.5）
}

/** 应用语言偏好（settings.json 的 locale 字段："system" 具体化）。 */
export function applyLocalePref(pref: string): void {
  const resolved = pref === "system" ? systemLocale() : pref;
  // createI18n 把 locale 推断成消息键联合，运行时值来自设置文件，放宽之
  i18n.global.locale.value = resolved as "zh-CN" | "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: systemLocale(),
  fallbackLocale: "zh-CN",
  messages: {
    "zh-CN": zhCN,
    en: en,
  },
});
