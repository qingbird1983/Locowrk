# 设计：Phase 5 WebView 混合渲染与标签页宿主

- 状态：定稿（2026-10-28 修订：双 WebView 前置 spike 已通过，见 §9）
- 日期：2026-10-28
- 关联：ADR-0014、ADR-0013、ADR-0002；architecture/04 §3.2、§3.4；roadmap/phase-5；research/03 §10、§11

## 1. 目标与非目标

**目标**：主区域能同时容纳“会话”和若干插件标签页；每个标签页是一张由宿主创建、摆放、销毁的系统 WebView 页面；Diff 审阅是第一个使用方。切换标签页不丢状态，宿主弹层能覆盖在网页之上，关 tab 即释放资源。

**不做**：第三方插件注册标签页、标签页持久化/拖拽排序/分屏、标签页内嵌宿主模态、全局快捷键、完整代码编辑器、在 WebView 里执行用户脚本。

## 2. 窗口与布局

```text
┌──────────────────────────────────────────────────────────────┐
│ 原生顶条：workflow（session.timeline.above）      [插件 ▾]   │
├──────────────────────────────────────────────────────────────┤
│ [会话] [变更：main.go ×] [变更：diff.go ×]              [⋯] │  ← 仅在有插件标签页时
├───────────────────────────────────────────┬──────────────────┤
│ 活动标签页的内容                          │ 原生右栏 260px   │
│ 会话 WebView                              │ ├ 编排树         │
│ 或 Diff WebView                           │ └ 工具检查器     │
├───────────────────────────────────────────┴──────────────────┤
```

- 标签栏 30 px，只在至少一个 Diff 标签页打开时出现；没有标签页时布局与 Phase 4 完全相同。
- 标签标题最多 24 个字符（按 rune 截断），活动标签用强调色下划线；每个标签右侧有 `×`。
- 标签栏右端 `⋯` 只在活动标签是 Diff 时显示：刷新、复制路径、关闭标签页。菜单由 `ui.Popover` 绘制。
- 右栏、顶条、分隔线保持 Phase 4 的规则；本阶段不调整视觉，只加功能。

## 3. 组成与依赖方向

```text
cmd/locowrk        装配主 WebView、Host、Diff 绑定；冒烟入口
desktop/tabs.go    标签页状态、标签栏、WebView 生命周期
desktop/diff*.go   git 读取、解析、Diff 绑定
web/src/diff/*     Diff 页面（第二张前端入口）
web/src/main.ts    工具卡片“查看变更”
```

- 依赖方向不变：`desktop` 可 import MyGo、`ui`、`internal/bridge|eventbus`；标签页与 Diff 不 import `internal/kernel`，不读会话文件。
- 页面只通过生成式客户端调用 `Desktop` / `Diff`；Go 不在运行期 `Eval` 页面（冒烟脚本例外）。
- 标签页是宿主服务，不是 `DesktopPlugin`；插件注册表与启停设置不变。

## 4. 标签页契约（`desktop/tabs.go`）

```go
type TabID string

// TabSession is the tab that shows the transcript page.
const TabSession TabID = "session"

type TabRequest struct {
    Key   string // 同一会话内稳定；重复打开只激活已有标签页
    Title string // 标签文案，宿主按 24 字符截断
    URL   string // 应用前端路径，必须以 "/" 开头
    Data  string // 业务数据键，宿主不解释；Diff 用它放工作区相对路径
}

type TabInfo struct {
    ID    TabID
    Key   string
    Title string
    URL   string
    Data  string
}

type TabHost interface {
    OpenTab(TabRequest) (TabID, error) // 任意 goroutine；禁止在 Build 中调用
    ActivateTab(TabID)
    CloseTab(TabID)
    Tabs() []TabInfo
    ActiveTab() TabID
    TabData(TabID) (TabInfo, bool)
}
```

- 上限 `maxWebTabs = 4`；错误哨兵：`ErrTooManyTabs`、`ErrTabURL`、`ErrTabClosed`。
- 会话标签固定存在且不可关闭；`Key` 只要求在同一会话内唯一。切换会话关闭全部插件标签页。
- `OpenTab` 用独立的 `tabMu` 串行化，**不得在持有宿主主锁时创建或销毁 WebView**：主线程构建帧时要拿主锁，而 `NewWebView`/`Destroy` 会切回主线程，持锁会死锁。
- 页面 URL 只接受 `/` 开头的应用路径；拒绝带 scheme 的地址，避免把标签页变成浏览器。
- 宿主快照（冒烟用）增加 `tabs` 数量、活动标签与“活动标签是否就绪”。

## 5. WebView 生命周期

1. **会话页**：由 `cmd/locowrk` 在窗口创建后 `win.NewWebView({URL: "/"})`，与 Phase 4 相同。
2. **插件标签页**：第一次 `OpenTab` 时创建 `win.NewWebView({URL: "/diff.html?tab=<id>"})`；创建一次，之后只摆放与显隐。
3. **切换**：`View` 只为活动标签构建 `ui.WebView(c, tab.Web)`；非活动标签不构建。MyGo 在元素消失时隐藏 WebView 但保留页面（research/03 §10），因此滚动位置、视图模式、选中文本都保留。
4. **关闭**：从状态中移除，随后 `Destroy()`；活动标签回退到左侧标签，左侧没有则回退 `会话`。关闭后再次打开同目标会新建 WebView。
5. **会话切换 / 窗口关闭**：先关闭所有插件标签页并销毁 WebView，再清理插件数据；窗口关闭时 `Host.Close` 负责兜底。
6. **创建失败**：记录 `tabs` 诊断，向调用方返回错误；已有标签页与其他插件不受影响。
7. **不持久化**：标签页不写设置文件；重启后只有 `会话`。

## 6. 资源、导航与 IPC

- 资源仍由 `mygo.SetFrontend(web.FS())` 提供；`diff.html`、`diff.js`、`diff.css` 进同一 `embed.FS`，运行期不读磁盘、不连网。
- `diff.html` 的 CSP 与主页面同源同策略：`default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'`。
- 新绑定：
  - `Desktop.OpenDiff(sessionID, toolCallID) error`（页面工具卡片调用）；
  - `Diff.Load(tabID) (DiffDoc, error)`、`Diff.CopyPath(tabID, path) error`（Diff 页面调用）。
- 统一走 `-generate-bindings` 生成的 `mygo.ts`；不手写第二套协议。生成后 `git diff` 只应出现上述方法与 DTO。
- 重复打开同一标签页时，宿主向该页发送 `diff-reload` 事件；这是唯一新增的页面事件，用于保留模式与滚动锚点地重新取数。
- 页面加载数据用“拉取”：Diff 页面 DOM 就绪后调用 `Diff.Load`；Go 不向隐藏页面推大块数据。
- 导航护栏：两张页面都装 `Page.OnWillNavigate`，只允许应用自身来源（开发期加 dev origin）；`http(s)` 外链 `PreventDefault` 后交 `mygo.Shell.OpenExternal`；`SetWindowOpenHandler` 拒绝新窗口。

## 7. 焦点与覆盖层

- 所有弹层由原生层绘制。标签栏 `⋯` 的 `ui.Popover` 位于 WebView 之后构建，MyGo 会把它算进 WebView 的 `covers`，因此显示在页面之上且能接收点击。
- 关闭弹层后焦点回到锚点按钮；`ui.WebView` 是一个 Tab 停靠点，再按一次 Tab 键盘进入页面。页面内部焦点由页面自己管理。
- 标签页内不实现会覆盖整个窗口的模态；需要确认时用原生弹层。
- Phase 5 不注册应用级快捷键（`Ctrl+W` 等留到 UI 重设计，因为 MyGo 的应用菜单加速键会引入菜单栏）；页面保留浏览器自身的滚动、选择、复制行为。

## 8. 帧纪律与主线程

1. `OpenTab` 只从绑定 goroutine、原生按钮回调或冒烟脚本调用；绝不在 `Build`/`View` 中调用。
2. `View` 只读快照：标签数量 ≤5，构建成本可忽略；不创建、不销毁 WebView。
3. `ui.Element` 不进入任何结构体字段；标签栏每帧重建。
4. 回调捕获 `Host`（长期有效），不捕获 `*ui.Context`。
5. 隐藏窗口不构建原生内容（P-008）：标签页状态与页面仍存在，但看不到标签栏；冒烟只断言数据与页面文本，渲染用 `ui.Tester`。

## 9. 测试与验收

| 检查 | 手段 | 断言 |
|---|---|---|
| 标签状态 | `desktop/tabs_test.go`（注入 WebView 工厂） | 打开/激活/关闭/去重/上限/会话重置 |
| 标签栏 | `ui.NewTester` + 零值 `*mygo.WebView` 句柄 | 标题、活动态、关闭按钮；非活动标签不摆放，活动标签摆放 |
| 生命周期 | 同上，工厂计数 | 每标签只创建一次；切换不重建；关闭即销毁；重开新建 |
| 覆盖层 | `ui.NewTester` | `⋯` 菜单文本出现；菜单区域落在 WebView 洞之上；关闭后焦点回锚点 |
| IPC | 生成器 | 只多出预期方法/DTO；`mygo.ts` 可复现 |
| 冒烟 | `-smoke-scenario diff` / `diff-tool` | 打开标签、页面含文件与增删行、关闭后 `tabs=0`；`diff-tool` 走工具卡片 |
| 导航 | 单元 | `allowedPageURL` 只放行 `mygo://localhost`；外链被 `PreventDefault` |
| 回归 | Phase 4 的 7 个场景 | 全部重跑通过，断言不放宽 |
| 人工 | 可见窗口 | 两标签切换、滚动保持、弹层覆盖、焦点回页面、高 DPI |

**开工前置（2026-10-28 已通过）**：`fixtures/spike-mygo-tabs` 验证了同窗两张 WebView、切换显隐、页面状态保留、`ui.Popover` 覆盖与 `Destroy`；结果与读数见 research/03 §11，弹层截图见 `design/phase-5/spike-tabs.png`。未覆盖焦点往返、中文输入法、高 DPI 与销毁后的进程回收，留给可见窗口人工验收。

**测试边界**：`ui.NativeWebView` 的方法参数含 MyGo internal 类型，仓库外不能实现假接口。无窗口测试使用零值 `*mygo.WebView` 句柄（`ui.Tester` 只比较身份与摆放），关闭/刷新由注入工厂计数；不要尝试实现 `ui.NativeWebView`。原生 `CapturePage` 不含子 WebView，不能用来证明弹层压住页面。

## 10. 风险与待决

- **双 WebView 行为已通过**：切换、页面状态、弹层覆盖、`Destroy` 均有证据（research/03 §11）；剩余未知是同窗焦点往返、中文输入法与高 DPI，作为可见窗口人工验收项，不再是设计前提。
- **每标签一个 WebView 的内存**：上限 4 个；关闭即销毁。若实测单个标签常驻过高，改为按需销毁非活动标签（会丢页面状态，需重新评估验收 3）。
- **标签栏是功能粗稿**：宽度、按钮尺寸、配色留到 UI 重设计，不在本阶段打磨。
- **焦点与输入法**：切换标签后键盘进入页面、中文输入需人工确认；若失败，记录为平台问题并评估 `Focus()` 调用时机。
- **隐藏窗口**：`-smoke` 看不到标签栏（P-008）；证据拆成数据、无窗口 tester 与可见窗口人工三部分。
- **语义变更**：`session.tab` 从“原生 Build 插槽”改为“宿主 WebView 标签页”已由 ADR-0014 记录；若未来要原生标签页，另加插槽，不复用本语义。
