# P-007：MyGo 页面窗口与原生窗口互斥，混排只能走原生窗口

- 现象：给 Phase 3 那种 `WindowOptions.URL` 窗口调用 `win.NewWebView(...)` 得到 `mygo: web views go in windows showing native UI (WindowOptions.Content)`（源码里的 `errNoContent`）；反过来，原生窗口里没有 `win.Page()` 可操作。
- 根因：MyGo v0.7.1 一个窗口只能是“页面窗口”或“原生 UI 窗口”之一。页面窗口自身就是一张网页，不能再嵌 WebView；原生窗口用 `ui.WebView` 元素把 `*mygo.WebView` 放进布局，`Window.NewWebView` 只在 `Content != nil` 时可用。
- 修复/做法：要“原生控件 + 网页”同窗，必须把窗口改成 `WindowOptions{Content: ui.View(...)}`，页面用 `win.NewWebView(...)` 加载，`ui.WebView` 摆放；页面操作与求值改用 `webview.Page()`。
- 预防：Phase 4 起主壳按 ADR-0013 走原生窗口；`Event.Emit(主窗口, …)` 仍会送到其所有 WebView（`ipc.go`），不需要改成逐页发送。写冒烟或新窗口代码前先确认窗口类型，别混用两种 API。
- 证据：`fixtures/spike-mygo-ui/`（2026-10-28 通过）；`docs/research/03-mygo-stack.md` §9。
