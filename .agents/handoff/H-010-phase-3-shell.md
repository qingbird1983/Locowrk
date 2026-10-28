# H-010：Phase 3 主壳落地

- 日期：2026-10-28
- 主题：`cmd/locowrk` + `desktop/` + `web/`；隐藏窗口冒烟通过

## 本轮做了什么

1. 按 `design/phase-3/shell.md` 与 `frontend.md` 实现：
   - `cmd/locowrk`：flags、kernel/bridge 装配、MyGo 窗口、`-smoke`、`-generate-bindings`。
   - `desktop/`：`App`（事件泵/窗口/关闭）与 `Bindings`（只暴露页面需要的 10 个方法）。
   - `web/`：TypeScript + esbuild、生成的 `mygo.ts`、内嵌 `dist/`；会话列表、消息流、工具卡片、输入/停止、自动滚动、错误提示。
2. 生成客户端：`go run ./cmd/locowrk -generate-bindings web/src/mygo.ts`；页面不手写 IPC。
3. 冒烟：`go run ./cmd/locowrk -smoke -cwd <tmp>`，隐藏窗口 + faux provider，输出 `{"ok":true,...}`，退出 0。
4. 依赖：MyGo v0.7.1 直连；`x/text`、`x/image` 因 MyGo 升到 v0.43.0 / v0.46.0，已更新 standards/04 与 research/01。
5. 同步 CHANGELOG 0.7.0、看板、Phase 3 文档、CONTEXT。

## 遗留与下一步

- Phase 3 验收：真实 key、工具过程、取消、重启恢复、空闲 CPU、5k 消息、安全。
- 中文 IME、钥匙串、安装包/签名、三平台冒烟。
- Phase 3 报告与 tag。

## 已提取的沉淀

- 无新增。
