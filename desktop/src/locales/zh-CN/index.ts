// locales/zh-CN/index.ts —— 语言包装配点（vue-i18n 的 messages 从这里取）。
// 拆前是单文件 locales/zh-CN.ts（1072/1090 行，两份行数错位没法逐行对账），批 F-22b 按职责切成 6 个域文件。
// 展开次序 = 域名字母序（与内容无关，只求稳定）；新增域文件必须同时在这里加 import 与展开，
// 漏一条 scripts/check-i18n.mjs 的「未 import / 未展开」断言当场判红（漏了会静默丢一整族文案）。
import chat from "./chat";
import commerce from "./commerce";
import developer from "./developer";
import settings from "./settings";
import shell from "./shell";
import tools from "./tools";

export default {
  ...chat,
  ...commerce,
  ...developer,
  ...settings,
  ...shell,
  ...tools,
} as const;
