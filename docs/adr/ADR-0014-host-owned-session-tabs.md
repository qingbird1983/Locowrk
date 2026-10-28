# ADR-0014：主区域标签页由宿主托管，每个标签页一张 WebView

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0002、ADR-0011、ADR-0013；architecture/04 §3；research/03 §10；design/phase-5/webview.md、diff.md

## 背景

Phase 4 在插槽表里预留 `session.tab`，当时按 `DesktopPlugin` 的构建式接口理解成“插件在原生视图里画一块区域”。Phase 5 要在这里放 Diff 审阅：长文本、行号、选中、滚动与语法着色。用原生 `ui.View` 画 10,000 行代价高，而 MyGo 的 WebView 才具备浏览器的文本选择与滚动。

但 `DesktopPlugin.Build` 的帧纪律不允许插件创建或持有 WebView：

- `ui.Element` 只在帧内有效；WebView 必须在视图外创建一次，视图只负责摆放与显隐（architecture/04 §3.4）。
- `Build` 每帧在主线程调用；创建 WebView 会阻塞，且资源要跨帧持有。

MyGo v0.7.1 的 WebView 是“一个页面一张视图”：页面在视图不显示时保留，同一原生窗口可以有多张（`webview.go`、`ui/webview.go`；见 research/03 §10）。

## 决策

1. `session.tab` 是**宿主托管的主区域标签页区**，不是 `DesktopPlugin.Build` 的插槽。`DesktopPlugin` 接口不变。
2. 宿主（`desktop` 的 `TabHost`）为每个打开的标签页创建一张 WebView，页面来自应用自身前端（如 `/diff.html?tab=…`）；标签页关闭时 `Destroy`，页面与原生视图资源随之释放（共享的 WebView2 运行时进程是否退出由系统决定，不承诺立即终止）。
3. 标签页的打开、关闭、激活由宿主 API 控制；Phase 5 的调用方是 Diff 审阅（工具卡片与宿主菜单），不是插件注册表。
4. 同一目标重复打开只激活已有标签页并请它刷新数据；最多同时打开 4 个；切换会话或关闭窗口时全部关闭。
5. 标签栏、关闭按钮、宿主菜单由原生层绘制，位于 WebView 之上；标签页内不实现与宿主冲突的模态，也不注册全局快捷键。
6. 页面状态（滚动位置、并排/统一模式）由页面自己保持：切走只是不再构建 `ui.WebView`，不销毁页面。

## 后果

- 页面状态天然保留，不需要在 Go 侧保存滚动位置或视图模式。
- 标签页数量直接对应 WebView 数量，内存有上限；用 4 个上限和“关闭即销毁”约束。
- `session.tab` 的文档语义需要修订：architecture/04 §3.2、design/phase-4/host.md 的待决 2。
- 无窗口测试要能把 WebView 换成假实现（`ui.NativeWebView` 只有一个方法），否则标签切换无法断言。
- 不影响 Phase 4 的 `DesktopPlugin`、`PluginHost` 与已有测试；原生插件与网页标签页并存。
- 暂不持久化标签页，不做拖拽排序、分屏或标签分组；需要时另立设计。

## 备选与否决

- **单 WebView 多路由**：切标签页要保存与恢复滚动、DOM，容易丢状态，还要维护路由与宿主标签两套身份。
- **原生 `Build` 画 Diff**：10,000 行文本的布局、选中与滚动成本高，Phase 5 选 WebView 正是为了避免这一点。
- **在 `Build` 里创建 WebView**：违反帧纪律，且每帧都可能新建，资源不可控。
- **插件自己持有 WebView**：插件状态从“只存数据”退化为持有界面资源，故障隔离与测试边界都变差。
