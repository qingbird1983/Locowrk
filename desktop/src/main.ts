import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { i18n } from "./i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./style.css";

// 启动白屏修复（2026-10-02 核查）：窗口 visible:false 起步（tauri.conf.json），
// 首帧渲染（mounted + 2×rAF）后由前端显窗——WebView2 初始化 + 模块加载期间
// 桌面不再露白；Rust 侧另有 8s 兜底显窗（lib.rs setup）。
const t0 = performance.now();
createApp(App).use(createPinia()).use(i18n).mount("#app");

requestAnimationFrame(() =>
  requestAnimationFrame(async () => {
    console.info(
      `[startup] first paint in ${Math.round(performance.now() - t0)}ms (main.ts entry → mounted+2rAF)`,
    );
    try {
      await getCurrentWindow().show();
      await getCurrentWindow().setFocus();
    } catch (e) {
      console.error("[startup] window show failed", e);
    }
  }),
);
