# 设计：Phase 4 插件宿主（PluginHost 与首批原生插件）

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0013、ADR-0002、ADR-0011；architecture/04 §3；roadmap/phase-4；[MyGo spike](../../research/03-mygo-stack.md#9-phase-4-spike-结果2026-10-28)

## 1. 目标与非目标

**目标**：在现有主壳里落地插件宿主与三个原生插件——编排树、workflow 时间线、工具检查器；验证插槽、事件分发、启停持久化、元素生命周期与故障隔离。

**不做**：第三方动态插件（Phase 7）、WebView 插件内容（Phase 5）、插件市场/签名/更新、把会话列表或消息流迁到原生、插件自定义命令与快捷键。

## 2. 窗口形态

按 ADR-0013，主窗口改为原生窗口，页面内嵌：

```
┌──────────────────────────────────────────────────────────────┐
│ 原生顶条：workflow 阶段条（session.timeline.above）  [插件 ▾] │
├────────────────────────────────────────┬─────────────────────┤
│                                        │ 原生右栏 260px      │
│  WebView：会话列表 / 消息流 / 输入区   │  ├ 编排树           │
│  （Phase 3 前端原样）                  │  │ session.sidebar  │
│                                        │  ├ 工具检查器       │
│                                        │  │ session.tool.detail
├────────────────────────────────────────┴─────────────────────┤
```

- 窗口：默认 1100×760，最小 900×620；右栏可折叠，折叠后页面占满。
- 天然分界用 1px 分隔线，不画卡片、不做渐变；跟随系统深浅色（`ui.Context.Theme`）。
- 顶条无内容（没有 workflow 步骤）时高度为 0，页面直接顶到窗口上沿。
- 右栏上下两段等分，各自可滚动；空状态显示“暂无”，不占位报错。
- 所有弹层（插件菜单、确认框、toast）由原生层绘制，显示在页面之上。

**插槽与布局的对应（Phase 4 契约）**

| 插槽 | Phase 4 位置 | 使用插件 |
|---|---|---|
| `session.sidebar` | 原生右栏上半 | `orchestration-tree` |
| `session.timeline.above` | 原生顶条 | `workflow-timeline` |
| `session.tool.detail` | 原生右栏下半 | `tool-inspector` |
| `session.tab` | 预留（主区域切换） | 无 |
| `settings.section` | 预留（原生设置面板） | 无 |

**与 architecture/04 §3.2 的修订说明（2026-10-28）**：`session.tool.detail` 原写“工具卡片展开区”，位于页面内部，原生层无法在滚动网页里就位。Phase 4 改为“右栏工具详情面板”：页面里的工具卡片只负责选中（`Desktop.SelectToolCall`），详情在原生栏显示。Phase 5 若需要页面内展开，由 WebView 插件承担。

## 3. 组成与依赖方向

```
cmd/locowrk        装配：内核 → bridge → App；创建原生窗口 + WebView；把事件同时给 App 和 Host
desktop/           MyGo 窗口、绑定、事件泵、原生外壳、PluginHost、三个插件
internal/bridge    DTO 门面（不变）
internal/eventbus  事件与 DTO（不变）
web/               前端（仅加“工具卡片可选中”）
```

- 依赖方向不变：`desktop` 可 import MyGo 与 `internal/bridge|eventbus`；插件不得 import `internal/kernel`，不得读会话文件。
- 插件是 `desktop` 包内的静态实现（Phase 4 不开放外部注册），但只经 `DesktopPlugin`/`PluginHost` 接口使用。

## 4. 接口

```go
// desktop/plugin.go
type Slot string

const (
    SlotSessionSidebar  Slot = "session.sidebar"
    SlotTimelineAbove   Slot = "session.timeline.above"
    SlotToolDetail      Slot = "session.tool.detail"
    SlotSessionTab      Slot = "session.tab"
    SlotSettingsSection Slot = "settings.section"
)

type DesktopPlugin interface {
    Name() string                       // 稳定 ID，写进设置文件
    Title() string                      // 界面文案（中文）
    Slot() Slot
    InterestedEvents() []eventbus.Kind
    Dispatch(eventbus.Event)            // 只改数据；可能在 bridge goroutine 上调用
    Build(c *ui.Context, h PluginHost)  // 只在主线程、每帧调用；不得保存 Element
}

type PluginHost interface {
    SessionID() string                  // 当前会话；插件按它重置数据
    SelectedToolCall() string           // 页面选中的工具调用 ID，空表示未选
    Invalidate()                        // 请求重绘（任意 goroutine 安全）
    Diagnostic(name string) string      // 该插件最近一次故障文本，空表示正常
}

type PluginDiagnostic struct {
    Plugin  string    `json:"plugin"`
    Code    string    `json:"code"`     // "panic" | "settings" | "payload"
    Message string    `json:"message"`
    At      time.Time `json:"at"`
    Count   int       `json:"count"`
}
```

- `Build` 不返回 `ui.Element`：MyGo 的元素只在帧内有效，构建式接口比返回值更不容易被缓存（architecture/04 §3.1 的 `View() ui.View` 同步修订为构建式）。
- `Dispatch` 里不解码 UI、不碰窗口；数据更新后调用 `Invalidate()`。
- 插件数据自带互斥锁；`Build` 在同一把锁下取快照，构建期间不持锁。
- 每个插件一个文件（`plugins_tree.go` / `plugins_timeline.go` / `plugins_tools.go`），单文件 ≤500 行。

## 5. 宿主（`desktop/host.go`）

**注册与启停**

- `NewPluginHost(opts)` 静态注册三个插件，顺序即右栏顺序：编排树 → 工具检查器。
- 启停状态来自设置文件；缺省全开。禁用的插件不接收事件、不占插槽、不显示标题。
- 切换启停立即生效并落盘；失败只记录诊断，不阻塞界面。

**事件分发**

- `Dispatch(ev)`：`Host` 按 `Kind` 查表（启动时建一次 `map[Kind][]DesktopPlugin`），只投给关心该事件的已启用插件。
- 会话不匹配的事件丢弃：`ev.SessionID != "" && ev.SessionID != h.sessionID` 时忽略。
- 批量合并：事件只更新数据并调用一次 `Invalidate()`；MyGo 把重绘压到下一帧，密集事件不会一事件一帧。
- 事件解码失败（payload 不是预期 JSON）：该插件记一条 `payload` 诊断（首次 + 次数），不影响其他插件。

**会话切换**

- `SetSession(id)`：清空所有插件数据与选中项，再通知各插件（`Reset()` 可选实现）。
- 由 `App.selectSession` 在 `Watch` 成功后调用；`Shutdown` 时调用 `SetSession("")`。

**故障隔离**

- `buildSlot(c, slot)` 对每个插件单独 `defer recover()`；panic 时：记 `panic` 诊断、把插件标记为 failed、下一帧起在该插槽显示“插件已停用（异常）”卡片，进程与其他插件继续工作。
- 已知代价：panic 发生的这一帧，插槽内的部分元素已创建、其后同帧的构建会看到 `building` 计数偏高。MyGo 每帧在 `context.reset` 重置该计数（源码可查），下一帧恢复正常；这属于“丢一帧”，不写回任何状态。
- failed 插件可经插件菜单“重试”手动恢复（清诊断、重新启用），不自动重启。

**设置文件**

- 路径：`os.UserConfigDir()/Locowrk/settings.json`（Windows 为 `%AppData%\Locowrk\settings.json`）；`-config` 可覆盖，测试与冒烟必须指到临时目录。
- 结构：`{ "version": 1, "plugins": { "<name>": true }, "rail": "open" }`。
- 读取：文件缺失 → 全默认；无法解析 → 全默认 + 一条 `settings` 诊断（不覆盖原文件，留待人工查看）。
- 写入：先写同目录临时文件再 `os.Rename`；只写已知插件名，未知键丢弃（前向兼容不承诺）。
- 不含凭证、路径、会话内容。

**冒烟快照**

- `Host.Snapshot()` 返回 `{sessionId, nodes, steps, tools, failed[], diagnostics[]}`，只给 `-smoke` 断言用，不暴露给页面。

## 6. 三个插件

### 6.1 `orchestration-tree`（`session.sidebar`）

- 事件：`subagent_spawned`、`subagent_settled`，外加 `tool_execution_end` 用于会话根节点的活动计数。
- 数据：根节点 = 当前会话；子节点 `{id, name, state, tools, lastTool, startedAt}`。
  - `subagent_spawned` payload 期望 `{id, name?, parentId?}`；缺 `id` 丢弃并记 `payload` 诊断。`parentId` 指向其他子代理时挂在其下（最多两层，超出挂根）。
  - `subagent_settled` 置 `done`；同 id 重复到达只更新时间。
  - `tool_execution_end`：根节点 `tools++`；`isError` 时 `failedTools++`；子节点归属信息缺失时不猜测。
- 界面上限：最多 200 个节点；超出时按“已结束且最旧”淘汰。每行显示状态点、名称（超长截断 + tooltip）、工具数。
- 空状态：`暂无子代理`。
- 交互：点击节点只做选中（高亮），不改变会话；Phase 4 不提供跳转。

### 6.2 `workflow-timeline`（`session.timeline.above`）

- 事件：`workflow_step`，payload `{name, status, detail?}`；`status ∈ pending|running|done|failed|skipped`。
- 数据：按第一次出现顺序排列的步骤；同名步骤原位更新；终态（`done`/`failed`/`skipped`）不被 `running` 覆盖。
- 界面：横向步骤条 `名称 › 名称 › …`，当前步骤强调色，`failed` 用危险色，`skipped` 弱化；最多显示 12 步，更多折叠为“+N”。
- 没有步骤时整条不占高度。
- 不做动画轮播；状态变化即时重绘。

### 6.3 `tool-inspector`（`session.tool.detail`）

- 事件：`tool_execution_start|update|end`。
- 数据：最近 200 次调用（环形缓冲）`{toolCallId, name, args, text, isError, durationMs, at}`；`update` 只累加进度文本；`end` 写结果、耗时与错误。
- 截断：参数与结果各 ≤2 KiB，超出加“…（截断）”；不写入会话文件。
- 界面：上半是调用列表（名称、耗时、失败标记）；下半是所选调用的参数与结果（等宽字体、可滚动）。
- 选中来源：原生列表点击，或页面工具卡片点击（`Desktop.SelectToolCall(sessionID, toolCallId)`）。页面只传 ID；宿主校验会话后记录选中并重绘。
- 空状态：`暂无工具调用`；有调用但未选中时显示“选择一条调用查看详情”。

## 7. 帧纪律

1. 插件状态只含数据（struct、slice、map），不含 `ui.Element`、`*mygo.Window`、channel。
2. `Build` 每帧从头构建；持久状态（选中、滚动位置、折叠）靠 `ui.Key` 或数据字段保持。
3. 回调捕获宿主对象（长期有效），不捕获 `*ui.Context`。
4. 大列表用 `ui.Scroll` + 行级 `Key`；Phase 4 的规模（≤200 行）不引入额外虚拟化。
5. 任何计算都不放在 `Build` 里做（结果在 `Dispatch` 阶段算好）；`Build` 只做布局与文案。
6. 不启动后台 goroutine、不读磁盘、不发网络请求。

## 8. 主壳与前端改动

**`desktop/shell.go`（原生外框）**

- `View(c)`：先取 `h.web`；未 Attach 时返回空，Attach 后 `win.Invalidate()`。
- 结构：`Row` →（`Column` 主区：顶条 + `WebView`）→（右栏：两个插槽段）。
- 顶条右侧：`插件 ▾` 按钮 + `ui.Popover`（三个勾选框、右栏开关、诊断行，“重试”按钮）；`ui.Checkbox` 的更改立即落盘。
- 右栏标题栏：名称 + 折叠按钮（`‹`/`›`），折叠状态写设置。
- 页面区域可加 `Radius(0)` 与 1px 边框，避免与原生背景拼色差异；不给页面设置内边距（会压缩前端布局）。

**`cmd/locowrk`**

- 创建顺序：`NewWindow(Content: ui.View(host.View))` → `NewWebView({URL: pageURL})` → `host.Attach(win, web)` → `app.SetWindow(win)` → `win.Invalidate()`。
- `OnEvent` 同时调用 `app.Emit(ev)` 与 `host.Dispatch(ev)`；顺序不影响（都不阻塞）。
- `-config` 新增；`-smoke` 默认把 config 指到临时目录。
- 冒烟等待与求值改用 `web.Page()`；`runSmoke` 结束时把 `host.Snapshot()` 合入结果 JSON。

**前端（`web/src/main.ts`）**

- 工具卡片增加可点击区域：点击调用 `Desktop.selectToolCall(sessionId, toolCallId)`，失败只写 `console`（不打断会话）。
- 不新增其他绑定；`mygo.ts` 由生成器重跑，`git diff` 只应出现 `selectToolCall`。
- 页面不再假设自己占满窗口；CSS 只在 `max-width: 720px` 下收窄会话侧栏（Phase 4 新增一条媒体查询）。

## 9. 测试与验收

**无窗口测试（`desktop/host_test.go`、`desktop/plugins_test.go`，`ui.NewTester`）**

| 检查 | 断言 |
|---|---|
| 插槽注册 | 三个插件各就位；禁用后该插槽不出现其标题 |
| 实时更新 | 注入 `subagent_spawned`/`workflow_step`/`tool_execution_*` 后一帧内文字出现，200 ms 预算远超 |
| 元素纪律 | `reflect` 遍历插件结构体，不得出现 `ui.Element` 类型字段；连建 60 帧无 panic、状态不丢 |
| 空状态 | 无事件时显示“暂无”，无报错 |
| 故障隔离 | 测试插件在 `Build` panic：其他插件仍出内容；该插件显示停用卡片；下一帧起不再调用它 |
| 设置 | 保存/读取往返；损坏 JSON → 默认值 + `settings` 诊断 |
| 性能 | 100 节点 + 500 事件：单帧构建均值 < 8 ms（spike 基线 0.18 ms，留量）；不出现帧率告警 |
| 边界 | `go list -deps` 断言插件不依赖 `internal/kernel` |

**集成冒烟（隐藏窗口）**

1. `-smoke-scenario plugin`：Phase 3 的 faux 会话跑通后，向 `host.Dispatch` 注入合成编排事件；等待 ≤200 ms，断言 `Snapshot()` 的三个计数与页面仍可继续发送下一轮。
2. `-smoke-scenario plugin-crash`（仅 `-smoke` 注册的故障插件）：首帧 panic，断言进程存活、其他插槽有内容、诊断可见、页面会话不受影响。
3. **回归**：`faux|real|tool|cancel|long|restore` 六个 Phase 3 场景全部重跑，断言不放宽。

**人工**

- 窗口形态与右栏折叠、插件勾选与重启保持；原生栏与页面之间点选、Tab、中文输入法；工具卡片点击后右栏选中；高 DPI 下 1px 分隔线不糊。

**验收对照（roadmap/phase-4 §5）**

| 条目 | 落点 |
|---|---|
| 1 插槽可见 / 启停保持 | §5 启停、§9 插槽注册与设置测试、人工重启 |
| 2 实时更新 200 ms | §9 实时更新、冒烟 1 |
| 3 元素纪律 | §7、§9 反射与多帧测试 |
| 4 性能 100 节点 + 500 事件 | §6 上限、§9 性能 |
| 5 隔离 | §5 故障隔离、§9 故障测试、冒烟 2 |
| 6 空状态 | §6 各插件空状态、§9 空状态 |
| 7 边界 | §3、§9 依赖检查 |

## 10. 风险与待决

- **迁移回归**：窗口形态变化影响 Phase 3 全部交互；六个冒烟场景是硬门禁。前端若有 fixed 定位的覆盖层，会在页面边界被裁剪，需在实现期逐一确认。
- **焦点/输入法**：原生栏与页面切换只在 Windows 验证；macOS/Linux 按 ADR-0007 只保证可构建。
- **panic 恢复的残余影响**：丢一帧、当帧计数偏移，已记录；若实测出现输入丢事件，改为“先探针构建、再正式构建”的双帧策略（见待决 1）。
- **`ui.WebView` 是 v0.7.1 新面**：行为以源码与上游测试为准；升级独立提交。
- **待决 1**：插件是否需要在 `Build` 前预检（无窗口 tester 构建一次）以把 panic 挡在帧外；Phase 4 先用插槽级 recover，够用则不引入双帧。
- **待决 2**：`session.tab` 与 `settings.section` 何时启用；Phase 5 的 Diff 插件会先用到 `session.tab`。
- **待决 3**：插件设置是否跟随会话或全局；Phase 4 定为全局（按插件名），与启停一致。
