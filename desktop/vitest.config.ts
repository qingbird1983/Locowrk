// 队列 Q11（前端测试基建）= 六份稿之外、队列自立的插入件：给 `stores/chat.ts` 的
// 行语义与组件层一个**能跑测试的运行面**，供 Q12（CF 稿 §10 · C1 数据链路）的
// 「重放回归」使用。
//
// ⚠ **为什么是独立配置文件，而不是往 `vite.config.ts` 里加一段 `test`**：
// `vite.config.ts` 是构建/dev 的正身（它钉着 `host: "::1"` 与 `strictPort` 两条本机
// 冷启动结论——IPv4 回环被过滤驱动拖 ~2s/连接，改回双栈 dev 首帧回到 30s+），
// 测试运行面是另一条通路，两者一起写会让"跑一次测试"依赖 dev server 的形状。
// 同一条纪律也解释了这里为什么只重复 `plugins: [vue()]`：SFC 要编译，
// 但 `server`/`build` 两段与测试无关，不继承。
//
// ⚠ **环境包只有 happy-dom 一个，不是"顺手再加 jsdom"**：`jsdom@30` 的 engines 是
// `^22.22.2 || ^24.15.0 || >=26.0.0`，本机 `node -v` 实测 **v24.14.0** ⇒ 不满足；
// `happy-dom@21` 要 `>=20` ✓。选型由版本号实测得出，不照教程抄。
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "happy-dom",
    include: ["tests/**/*.spec.ts"],
    // 不引 `globals: true`：`tsconfig.json` 的 `types` 只有 `vite/client`，开了全局
    // 就得再改 tsconfig（一处测试配置牵动构建侧类型面）。用例一律显式
    // `import { describe, it, expect } from "vitest"`。
  },
});
