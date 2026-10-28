# P-009：MyGo 的 `ui.NativeWebView` 在仓库外无法实现假接口

## 现象

在 `desktop` 测试里为标签页写 `type fakeWebView struct{}` 并实现 `SurfaceWebView(*surface.Conn) platform.WebView`，编译直接失败：`surface` 是 MyGo 的 internal 包，`desktop` 不能 import；不实现该方法的类型也不能赋给 `ui.NativeWebView`。真窗口又无法用于无窗口帧测试。

## 根因

`ui.NativeWebView` 是 `internal/surface.WebView` 的类型别名，方法参数 `*surface.Conn` 属于 MyGo 模块的 internal 路径。Go 的 internal 规则不允许外部模块命名该类型，因此外部代码无法声明完全匹配的方法。

## 修复

- 无窗口测试用**零值 `*mygo.WebView` 句柄**（`new(mygo.WebView)`）：`ui.WebView` 只保存它，`ui.Tester` 的 `WebViewAt` 只比较身份与摆放，不调用其方法。
- 标签页宿主注入 WebView 工厂：生产返回真实 WebView，测试返回零值句柄 + 计数回调（关闭/刷新/取 Page 不落到真实对象）。
- 不要为测试去实现 `ui.NativeWebView`，也不要为了可测性把宿主 API 改成接口再包装。

## 预防

- 设计测试方案时先核对依赖的接口是否含 internal 类型；`go doc` 看不到 internal 参数时按“不可实现”处理。
- 真实层级/焦点/销毁行为放到可见窗口 spike 或人工验收；无窗口测试只断言状态与摆放。
- 原生 `CapturePage` 不含子 WebView，不能证明弹层压在页面上；用 OS 截屏或人工确认。

证据：`research/03` §11；夹具 `fixtures/spike-mygo-tabs`（headless 模式）。
