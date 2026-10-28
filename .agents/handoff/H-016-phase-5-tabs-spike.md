# H-016：Phase 5 双 WebView spike 通过

- 日期：2026-10-28
- 主题：验证 ADR-0014 的前提（同窗多 WebView、隐藏保留、弹层覆盖、销毁）

## 本轮做了什么

- 新建 `fixtures/spike-mygo-tabs/`：A/B 两张页面（A 有滚动与输入状态），原生标签条 + `ui.Popover`，只摆放活动页。
- 三档验证：`-headless`（tester 断言切换与覆盖）、隐藏窗口（页面加载/状态/销毁）、`-show`（可见切换、原生捕获、OS 截屏）。
- 结果：切换只摆放活动页；A 的 `scrollTop=320` 与输入 `hello` 切回仍保持；B 检查值 `b`；`Destroy` 后 `IsDestroyed=true`；弹层在 OS 截屏中压在网页上。证据 research/03 §11、`design/phase-5/spike-tabs.png`。
- 发现测试边界：`ui.NativeWebView` 含 MyGo internal 类型，仓库外无法实现假接口；沉淀 P-009，改用零值句柄 + 工厂。
- 同步 Phase 5 设计/路线图状态、research/03、CHANGELOG 0.9.4、CONTEXT。

## 关键读数

- 隐藏窗口 ready 约 0.6 s；可见窗口 ready 约 1.1 s、弹层 2.3 s、销毁 4.0 s（含刻意 `-hold` 前）。
- 原生 `CapturePage` 18,287 B；页面区域为空白，不含子 WebView，不能当层级证据。
- 退出时 WebView2 打印 `Failed to unregister class Chrome_WidgetWin_0`，不影响结果。

## 遗留与下一步

1. 按设计实现 `desktop/tabs.go` → `diff*.go` → `web/diff.html` → 工具卡片接线。
2. 可见窗口人工：两页焦点往返、中文输入法、高 DPI。
3. 未测：`Destroy` 后进程/内存归零，只断言 `IsDestroyed`。

## 已提取的沉淀

- P-009：MyGo `ui.NativeWebView` 无法在仓库外实现假接口，测试用零值句柄。
- 沿用 P-008：隐藏窗口不构建原生内容；所以布局/覆盖必须 `-show` 或 tester。
