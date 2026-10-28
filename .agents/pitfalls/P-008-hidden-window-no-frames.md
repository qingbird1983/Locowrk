# P-008：隐藏窗口不画原生内容，内嵌页面的 rAF 也会暂停

- 现象：`-smoke`（隐藏窗口）下，原生插件栏与顶条从不构建（`Host.View` 0 次），插件里的 panic 永远不触发；同时内嵌 WebView 的页面收得到事件，但 transcript 的 DOM 一直不更新，`requestAnimationFrame` 永不回调，冒烟超时。
- 根因：Windows 对隐藏/遮挡窗口不派发绘制（MyGo 的原生内容只对可见窗口构建帧）；WebView 的页面在窗口不可见时被浏览器引擎判定为 occluded，rAF 暂停。Phase 3 的“页面窗口”没有这个问题，切到内嵌 WebView 后才暴露。
- 修复：① 前端合批改为 rAF + 120 ms 定时器兜底，rAF 触发时清掉定时器（否则 `pendingTimer` 会挡住后续合批）；② 验收拆分：隐藏冒烟只断言插件数据，渲染与 panic 隔离用 `ui.NewTester`，`plugin-crash` 场景用 `-show` 显示窗口（Windows 下自动开启）。
- 预防：不要用隐藏窗口断言原生渲染或 DOM 挂载；页面测试等 `.tool` 等元素出现，不要假设下一帧已落 DOM。判断“原生外壳是否构建”看 `Host.Snapshot().Frames`。
- 证据：`docs/design/phase-4/report.md` §4；`cmd/locowrk -smoke -smoke-scenario plugin-crash`（`frames=4`）。
