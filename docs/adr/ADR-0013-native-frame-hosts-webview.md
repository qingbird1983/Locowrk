# ADR-0013：Phase 4 主窗口改为“原生外框 + 内嵌 WebView”

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0002、ADR-0011、ADR-0012；research/03 §9；roadmap/phase-4；design/phase-4/host.md

## 背景

ADR-0011 让 Phase 3 主壳走 WebView 页面窗口（`WindowOptions.URL`），整个窗口是一张网页。Phase 4 要在同一窗口里放原生 `ui.View` 插件插槽。

MyGo v0.7.1 的两种窗口互斥（源码可查）：

- **页面窗口**（`WindowOptions.URL` / `SetFrontend` 后自动打开）：`Window.NewWebView` 返回 `errNoContent`，窗口内不能再放原生视图。
- **原生窗口**（`WindowOptions.Content: ui.View(...)`）：`NewWebView` 可用，页面由 `ui.WebView` 元素放进布局，原生控件在其周围绘制、弹层覆盖在页面之上（`examples/embed-webview`）。

因此“保留现有网页 + 加入原生插槽”只有一条可行路径：把主窗口从页面窗口改成原生窗口，页面改为内嵌 WebView。这是语义级变更，ADR-0011 已把“主壳是否迁原生”留到 Phase 4 后评估；本 ADR 只决定**窗口形态**，不决定长期是否把会话列表/消息流也迁到原生。

## 决策

Phase 4 起主窗口采用 **原生外框 + 内嵌 WebView**：

- 窗口由 `mygo.NewWindow(WindowOptions{Content: ui.View(shell)})` 创建；`MinWidth/MinHeight` 计入原生栏。
- 现有前端（会话列表、消息流、输入区）**原样**加载进 `win.NewWebView(WebViewOptions{URL: "/"})`，由 `ui.WebView` 放在主区域。
- 原生外框只承载：顶部 `session.timeline.above` 条、右侧插件栏（`session.sidebar` + `session.tool.detail`）、插件菜单与诊断；不重做 Phase 3 的会话界面。
- 弹层、对话框、菜单、toast 由原生层统一管理；页面内不再新增会与原生层冲突的模态。
- 页面仍是前端唯一入口：绑定方法、`mygo.NewEvent` 事件与 `SetFrontend` 资源都不变；`Event.Emit(主窗口, …)` 会送到该窗口下所有 WebView（源码 `ipc.go`）。

## 依据（2026-10-28 实测）

`fixtures/spike-mygo-ui`（独立 module，MyGo v0.7.1，隐藏窗口）：

| 检查 | 结果 |
|---|---|
| 原生 `Content` 窗口 + `NewWebView("/")` | 通过；窗口 74.6 ms、WebView 93.3 ms（进程启动起算） |
| 页面 → Go 绑定调用 | 通过；679.5 ms |
| Go → 页面事件 → Go 回调 | 通过；684.4 ms（事件段约 5 ms） |
| 原生栏 + 页面同帧布局、原生弹层锚定 | 通过（布局构建无 panic；覆盖行为由上游 `ui/webview_test.go` 覆盖） |
| 无窗口多帧测试（100 行 + 500 事件计数） | 通过；120 帧平均 0.18 ms、最慢 0.65 ms |

## 后果

- **冒烟脚本要改入口**：页面不再属于窗口本身，`win.Page()` 改为 `web.Page()`；`-smoke` 的等待与 `EvalAs` 目标随之调整。
- **页面可用宽度变小**：原生右栏默认 260 px；窗口默认宽度 1100、最小 900。前端要在 ~620 px 下不破版（Phase 3 的布局是 flex，已复核）。
- **焦点/输入法多一个边界**：原生栏 ↔ 页面切换由 MyGo 处理（Tab 在 `ui.WebView` 停一次），但中文输入法必须重新人工确认。
- **覆盖层归属**：页面内的 fixed 弹层只覆盖页面区域；需要覆盖整个窗口的交互一律走原生。
- **回归面变大**：Phase 3 的 6 个冒烟场景必须在原生外框下全部重跑，任何失败按“迁移引入”处理，不得放宽断言。
- 长期是否把会话列表/消息流迁到原生，仍按 ADR-0011 在 Phase 4 结束后用新 ADR 决定；本 ADR 不预设结论。

## 护栏

- 前端不得依赖“自己拥有整个窗口”：不加窗口级 drag region、不做全窗口模态、不假设 `window.innerHeight` 等于窗口高度。
- 原生外框只读取 `bridge`/`eventbus` 的 DTO；不 import `internal/kernel`，不直接读会话文件。
- `ui.Element` 不进入插件状态；插件只保存数据，帧内构建。
- 插件 panic 不得杀死进程或主壳：在插槽级 recover，停用该插件并显示诊断（帧纪律见设计稿 §7）。
- 不升级 MyGo 版本；`ui.WebView` 用法收敛在 `desktop/`，升级独立提交。
