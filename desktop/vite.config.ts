import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// tauri.conf.json 的 devUrl 指向 5173，端口必须钉死
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "es2021",
  },
});
