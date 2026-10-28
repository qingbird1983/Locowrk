# spike-mygo-tabs —— Phase 5 双 WebView 前置验证

独立 module（MyGo v0.7.1），不参与主构建。目的：在写 Phase 5 功能前验证“同一原生窗口两张 WebView”这条前提。

## 跑法

```bash
cd fixtures/spike-mygo-tabs
go run . -headless                     # 无窗口：标签切换 + 弹层覆盖几何
go run .                               # 隐藏窗口：两张页面加载、状态、销毁
go run . -show -hold 4s -out tabs.png  # 可见窗口：切换、弹层、原生捕获
```

- `-headless` 用零值 `*mygo.WebView` 当句柄：`ui.Tester` 只比较身份与摆放，不调用其方法。MyGo 的 `ui.NativeWebView` 方法参数是 internal 类型（`surface.Conn`），仓库外无法实现假接口。
- `-show` 会短暂显示 900×600 窗口；`-hold` 让弹层在原生捕获后保持打开，便于同时截屏。
- 退出时 WebView2 可能打印 `Failed to unregister class Chrome_WidgetWin_0`，不影响结果，见 research/03 §11。

## 2026-10-28 结果（Windows 11，MyGo v0.7.1）

| 模式 | 结果 | 读数 |
|---|---|---|
| `-headless` | 通过 | 切到 B 只摆放 B；`菜单` 弹层覆盖页面（点所在位置不返回 WebView）；点“刷新数据”回调 1 次 |
| 隐藏窗口 | 通过 | 两页 ready 约 0.6 s；A 的 `scrollTop=320` 与输入值 `hello` 保持；B `Destroy` 后 `IsDestroyed=true` |
| `-show` | 通过 | 6 帧；A/B 切换后 A 状态仍为 `320\|hello`、B 为 `b`；原生捕获 18,287 B；B 销毁成功；ready 1.1 s、弹层 2.3 s、销毁 4.0 s |
| OS 截屏 | 通过 | 弹层（刷新数据/关闭标签页）画在网页之上，见 `docs/design/phase-5/spike-tabs.png` |

## 结论与边界

- 同窗两张 WebView、只摆放活动页、隐藏页保留状态、原生 `ui.Popover` 覆盖页面、关闭即销毁：**均通过**，ADR-0014 的前提成立。
- 原生 `CapturePage` 只捕获原生内容，不含子 WebView（图片中页面区域为空白）；弹层是否真正压在页面上，用 OS 截屏确认。
- 未覆盖：中文输入法、高 DPI、焦点在两张页面间往返、关闭后进程/内存是否归零（只断言 `IsDestroyed`）。这些留给 Phase 5 验收的可见窗口人工项。
