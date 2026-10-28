# H-013：Phase 4 实现与自动验收

- 日期：2026-10-28
- 主题：原生外框迁移、PluginHost、三个插件、冒烟与回归

## 本轮做了什么

1. 主窗口迁移：`Content: ui.View(host.View)` + `win.NewWebView("/")`；`App.SetPage` + `Event.EmitPage` 让内嵌页面收事件；`-config`/`-show`；`win.Page()` 不再存在。
2. 宿主：`desktop/plugin.go|host.go|settings.go`；按 Kind 分发、会话切换清空、插槽级 recover、设置原子写、`Snapshot`（含 `frames`）。
3. 外壳与插件：`shell.go`（顶条 + 右栏 + 插件菜单）、`plugins_tree.go`（200 节点）、`plugins_timeline.go`（6 步）、`plugins_tools.go`（200 条、2 KiB 截断）；工具卡片点击 → `selectToolCall`。
4. 前端：rAF + 120 ms 定时器兜底（P-008）；窄宽度媒体查询；生成客户端只多 `selectToolCall`；`dist` 重建入库。
5. 验收：`go test -race ./...` 全绿；100 节点 + 500 调用 0.70 ms/帧；`faux/tool/cancel/long/restore/plugin/plugin-crash` 全部通过；`real` 未跑（无 key）。
6. 文档：Phase 4 报告（草稿）、design/phase-4/host.md 按实现修订、architecture/04 接口与插槽表、roadmap 状态、CHANGELOG 0.9.1、P-008。

## 遗留与下一步

- 人工：可见窗口的栏宽/分隔线/高 DPI、原生栏 ↔ 页面的焦点与中文输入法；确认后把报告转定稿并打 `phase-4.end`。
- 有 key 时补跑 `-smoke -smoke-scenario real`。
- 复查 `standards/08 §6` 的“禁止提交 dist/”与前端 embed 入库的冲突（留发布工程一并处理）。

## 已提取的沉淀

- P-008：隐藏窗口不画原生内容，内嵌页面 rAF 暂停；验收按“数据 / 渲染”拆开。
