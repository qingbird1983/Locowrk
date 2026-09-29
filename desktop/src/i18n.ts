// vue-i18n 装配（DESIGN.md §13.5）：文案集中在 locales/，组件不硬编码。
// 当前语言缺省跟随系统语言、无匹配回落中文；持久化与切换入口随阶段八
// 设置面板落地。
import { createI18n } from "vue-i18n";
import en from "./locales/en";
import zhCN from "./locales/zh-CN";

function initialLocale(): string {
  const lang = (navigator.language ?? "").toLowerCase();
  if (lang.startsWith("zh")) return "zh-CN";
  if (lang.startsWith("en")) return "en";
  return "zh-CN"; // 无匹配回落中文（§13.5）
}

export const i18n = createI18n({
  legacy: false,
  locale: initialLocale(),
  fallbackLocale: "zh-CN",
  messages: {
    "zh-CN": zhCN,
    en: en,
  },
});
