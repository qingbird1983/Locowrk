import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { i18n } from "./i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";
import "./style.css";

// 启动白屏修复（2026-10-02 核查，2026-10-03 加固）：主窗 visible:false 起步，
// 首帧渲染（mounted + 2×rAF）后由前端显窗——WebView2 初始化 + 模块加载期间
// 桌面不露白；显窗后发 `lw:frontend-ready` 让 Rust 关掉启动闪屏（splash.html）；
// Rust 侧另有 25s 兜底（lib.rs setup）。
const t0 = performance.now();
createApp(App).use(createPinia()).use(i18n).mount("#app");

// shiki 高亮后台预热（§6.5 v4.16）：动态 import 不阻塞首帧，
// 首个报告渲染时通常已就绪；未就绪时代码块降级单色、流式重渲自愈。
import("./lib/highlight").then((m) => m.warmupHighlighter());

// 原生右键菜单（WebView2/Chromium 的前进/后退/检查等）与界面无关联（用户
// 2026-10-04 裁定隐藏）：全局禁用 contextmenu 默认行为；仅输入框/文本域
// /contenteditable 保留——复制/粘贴还要用。
document.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement | null;
  if (!el?.closest("input, textarea, [contenteditable]")) e.preventDefault();
});

requestAnimationFrame(() =>
  requestAnimationFrame(async () => {
    console.info(
      `[startup] first paint in ${Math.round(performance.now() - t0)}ms (main.ts entry → mounted+2rAF)`,
    );
    try {
      await getCurrentWindow().show();
      await getCurrentWindow().setFocus();
      await emit("lw:frontend-ready");
    } catch (e) {
      console.error("[startup] window show failed", e);
    }
  }),
);
