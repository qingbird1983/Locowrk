import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// tauri.conf.json 的 devUrl 指向 5173，端口必须钉死
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // 冷启动核查（2026-10-03）：本机 IPv4 回环每个 TCP 连接被过滤驱动拖
    // ~2s（127.0.0.1 = 2.02s vs ::1 = 0.02s，实测；疑 clash TUN/杀软钩子），
    // vite dev 数百个模块请求被拖成 30s+ 首帧。只绑 IPv6 回环，dev 加载
    // 整体绕开被过滤的栈（生产构建不受影响——无 vite，单文件产物）。
    host: "::1",
  },
  build: {
    target: "es2021",
  },
});
