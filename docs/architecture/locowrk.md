# Locowrk：Go + MyGo 复刻 Pi 桌面端（插件层完整设计方案）

- 状态：定稿（方案基线 v1.1）
- 日期：2026-10-28（立项方案；正文为 v1.1 原始内容）
- 关联：ADR-0001 ~ 0008；architecture/01 ~ 05；research/01 ~ 05

> 阅读提示：本文是立项方案与设计来源。系统分层、接缝、插件与会话的规范化描述分别在 `01-overview.md`、`03-critical-seams.md`、`04-plugin-system.md`、`05-session-and-resources.md`；若本文与后续 ADR 冲突，以 ADR 为准。
>
> **2026-10-28 注**：插件 UI 调用策略已由 ADR-0003 修订为“三分类 + no-op + 诊断”。本文 §三、§五、§八中“拒绝 / `ErrUIUnsupported`”是 v1.1 的原始表述，实现以 ADR-0003 与 architecture/04 为准。
>
> **2026-10-28 注**：内核来源已由 ADR-0012 改为仓库内 fork（`pi/`），本文“依赖 `github.com/sky-valley/pi` 作为库”的表述属于 v1.1 原文；当前以 `pi/` + `internal/kernel` 封装为准（architecture/02）。

**版本**：v1.1 · **目标**：以 Pi（earendil-works/pi）为行为契约，用纯 Go 内核 + MyGo 桌面宿主，构建一套“编排型插件选择性兼容、桌面端插件原生重建、插件具备完整界面渲染能力”的可落地架构。

> v1.1 核心增补：桌面端插件如何**真正画出界面**。MyGo 的窗口可以混合两种渲染路径——纯 Go 原生 `ui.View`（GPU 直接绘制，不经 HTML）与系统 WebView 内嵌网页。桌面插件将按需选择路径，宿主提供统一挂载点。


## 一、设计前提与核心判断

Pi 本身是极简 TUI Agent，默认只有 4 个工具（read/write/edit/bash），所有扩展能力通过四层体系提供：Extensions（TypeScript 插件）、Skills、Prompt Templates、Themes。复刻 Pi 到桌面端，真正的复杂度不在内核，而在**插件层的语义翻译与 UI 环境适配**。

本方案的核心判断有四条：

1. **内核直接复用，不自研**：`sky-valley/pi` 是忠实的纯 Go 移植，直接作为内核库依赖，把精力集中在插件层。
2. **编排型插件“逻辑借用、UI 剥离”**：保留子代理委派、workflow 状态机、异步编排等与 TUI 无关的逻辑；过滤 `ctx.ui.*` / `registerShortcut` 等 TUI 专属调用。
3. **桌面端插件“另起炉灶”，且具备完整画界面能力**：桌面插件围绕 Go 内核事件流和 MyGo 的 UI 能力重新构建。MyGo 的 `ui.View` 由 GPU 直接绘制，不走 HTML；同时窗口可以内嵌系统 WebView（`ui.WebView`），让需要富交互的插件走前端渲染。**插件画界面不是附属能力，而是宿主层的一等公民。**
4. **插件动态加载走进程隔离，不走 Go `plugin` 包**：Go 原生不支持安全的运行时动态加载/卸载，`plugin` 包仅限 Linux/macOS、禁止重载、要求版本严格匹配。生产级方案是**子进程 + IPC**，插件以独立进程运行，通过 stdin/stdout 或 gRPC 与宿主通信。


## 二、总体架构

```
┌───────────────────────────────────────────────────────────────┐
│  MyGo Desktop Host（桌面进程）                                  │
│                                                               │
│  ┌──────────────────────────┐  ┌──────────────────────────┐  │
│  │ 桌面插件宿主               │  │ MyGo 内置 UI              │  │
│  │ · PluginHost（插槽管理）   │  │ 会话流 / 输入 / 设置      │  │
│  │ · DesktopPlugin A/B/C     │  │ ui.View 原生渲染          │  │
│  │   ├─ 原生 ui.View 路径    │  │ 或系统 WebView            │  │
│  │   └─ WebView 前端路径     │  └──────────────────────────┘  │
│  └────────────┬─────────────┘                                │
│               │ MyGo Typed IPC（Bind + channel 事件流）        │
│  ┌────────────▼───────────────────────────────────────────┐  │
│  │  Go Harness Core（单二进制内）                          │  │
│  │  Agent Loop (sky-valley/pi) ←→ EventBus ←→ Session Store│  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │ Plugin Adapter Layer                             │  │  │
│  │  │ · OrchestrationAdapter（编排插件兼容 + UI 过滤）  │  │  │
│  │  │ · SkillLoader / PromptLoader（直接复用）          │  │  │
│  │  └──────────────────────────────────────────────────┘  │  │
│  └────────────┬───────────────────────────────────────────┘  │
│               │ 子进程 / gRPC / JSON-RPC（进程隔离）           │
│  ┌────────────▼───────────────────────────────────────────┐  │
│  │ Pi 编排插件运行时（独立进程）                           │  │
│  │ pi-orchestrator / pi-orchestration（TS 桥接或 Go 重写）│  │
│  └────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

整个系统是**单 MyGo 二进制**，Go 内核与 MyGo UI 同进程，通过 Typed IPC 通信；Pi 编排插件作为**子进程**运行在进程隔离的插件运行时中；桌面原生插件既可以是**宿主内编译的 Go 模块**（静态注册），也可以是**独立子进程**（动态加载，通过 IPC 向宿主返回 UI 描述或直接渲染到独立窗口）。


## 三、插件分类与兼容策略矩阵

| 插件层 | 典型内容 | 桌面端策略 | 理由 |
|---|---|---|---|
| **Extensions — 编排逻辑** | 子代理委派、workflow、异步编排 | **选择性兼容**：保留编排事件，过滤 UI 调用 | 逻辑与 TUI 无关 |
| **Extensions — UI/快捷键** | fullscreen overlay、keyboard nav | **排除**：`ctx.ui.*` / `registerShortcut` 拒绝或降级为空操作 | 终端语义在桌面无意义 |
| **Extensions — 自定义工具** | 插件注册的 tool | **兼容**：tool 输入输出为结构化数据即可 | 工具与 UI 正交 |
| **Skills** | `SKILL.md` + 脚本 | **直接复用** | 纯 Markdown，无 UI 依赖 |
| **Prompt Templates** | `.md` 模板 | **直接复用** | 纯文本 |
| **Themes** | 终端颜色 token | **丢弃** | 桌面视觉由 MyGo 主题系统掌控 |

Pi 的 Extensions 在 `ExtensionAPI` 层面做能力裁剪：允许 `on` / `registerTool`，拒绝 `registerShortcut` 和 `ctx.ui.*`。**桌面端不翻译 TUI 组件，而是用 MyGo 原生重建同等信息的可视化。**


## 四、Go 内核层

### 4.1 内核选型

依赖 `github.com/sky-valley/pi` 作为库：`ai`（多 Provider LLM API）、`agent`（Agent Loop、工具调用、steering/follow-up 队列）、`coding`（编码 Agent、JSONL 会话持久化）。内核不直接暴露给 MyGo UI，只通过 EventBus 输出统一事件。

### 4.2 统一事件模型

```go
type EventKind string

const (
    EventMessageDelta    EventKind = "message_delta"
    EventToolCallStart   EventKind = "tool_call_start"
    EventToolCallEnd     EventKind = "tool_call_end"
    EventSubagentSpawned EventKind = "subagent_spawned"
    EventSubagentSettled EventKind = "subagent_settled"
    EventWorkflowStep    EventKind = "workflow_step"
    EventSteeringApplied EventKind = "steering_applied"
)

type Event struct {
    Kind      EventKind
    SessionID string
    Payload   json.RawMessage
    Timestamp time.Time
}
```

EventBus 为简单 pub/sub，MyGo 侧通过 `HarnessService.SubscribeEvents` 返回 `<-chan Event`，Typed IPC 原生桥接 channel 流。


## 五、插件适配层：编排型插件的兼容实现

### 5.1 进程隔离

推荐 **子进程 + JSON-RPC over stdio** 或 **`hashicorp/go-plugin` over gRPC**。Go 的 `plugin` 包不可用于生产插件系统：仅 Linux/macOS、禁止重载、要求主程序与插件用完全相同的 Go 版本和编译器标志构建。进程隔离同时带来天然的资源限制和崩溃隔离能力。

### 5.2 适配器接口

```go
type OrchestrationAdapter interface {
    Load(ctx context.Context, pluginPath string) (*PluginManifest, error)
    Start(ctx context.Context, manifest *PluginManifest) error
    Events() <-chan Event
    SendCommand(cmd PluginCommand) error
    Tools() []ToolDefinition
    Stop(ctx context.Context) error
}

type PluginManifest struct {
    Name         string
    Version      string
    Capabilities []Capability // "orchestration" | "tool" | "ui" | "shortcut"
    UIRequired   bool
}
```

**过滤规则**：`Capabilities` 含 `ui` 或 `shortcut` 时，适配器在 IPC 层面**拒绝**对应调用（返回 `ErrUIUnsupported` 或结构化默认值），编排事件和工具注册照常工作。


## 六、桌面插件宿主与界面渲染（v1.1 核心章节）

### 6.1 MyGo 的两条渲染路径

MyGo 的每个窗口可以选择两种界面之一，同一窗口也可以混合两者：**系统 WebView 网页**（WKWebView / WebView2 / WebKitGTK，不打包浏览器引擎）或**纯 Go 原生 UI**（`ui.View`，由 MyGo 在 GPU 上用 Metal / Direct3D / OpenGL 绘制，或用 CPU 渲染，不经过 HTML）。

桌面插件的画界面能力因此分两条路径：

**路径 A — 原生 `ui.View` 插件（推荐用于编排可视化）**

插件用 Go 代码构建 `ui.View`，宿主将其嵌入窗口的插槽。`ui` 包提供 flexbox/grid 布局、控件、虚拟化列表、SVG 图标、动画、GPU 绘制，以及可脱离真实窗口的测试器。编排树、workflow 时间线、工具调用时间线这类结构化可视化，原生路径表达力充足且零 Web 开销。

**关键约束（必须写进插件契约）**：`ui.View` 是**应用状态的函数**，在**主线程**上构建一帧界面；`ui.Element` 在下次构建前过期，**不得存入应用状态或后台 goroutine**；持久状态（焦点、滚动、编辑中文本）靠元素在兄弟中的位置或 `Key` 保持；应捕获 `Services` 用于持久回调，而非存储 `Context`。插件应只持有**数据状态**，“怎么画”留在 `View` 函数里。

**路径 B — WebView 插件（推荐用于富交互面板）**

插件窗口使用系统 WebView，前端通过 MyGo 生成的 TypeScript 客户端调用 Go 方法、消费事件流。MyGo 的 `ui.WebView` 允许在**原生 UI 窗口内嵌入网页**，与原生控件并列渲染，弹出层（菜单、对话框、tooltip、toast）可覆盖在网页之上。适合 Diff 并排审阅、富文本编辑、图表库等场景。WebView 应**在视图外创建一次**，视图只负责展示它。

### 6.2 PluginHost：宿主层的插槽与挂载机制

MyGo 本身没有内置“插件 UI 插槽”原语。`PluginHost` 是**宿主层必须自行实现的抽象**：维护 `map[string][]PluginView`，在布局时按 slot 名称把插件视图嵌入对应区域。

```go
type DesktopPlugin interface {
    Name() string
    InterestedEvents() []EventKind
    // 返回原生 View；若走 WebView 路径，返回 nil 并实现 WebViewFactory
    View(ctx *ui.Context, host PluginHost) ui.View
    WebViewFactory() (ui.View, bool) // 可选：WebView 路径
    OnEvent(e Event)
}

type PluginHost interface {
    Subscribe(kinds []EventKind) <-chan Event
    Dispatch(cmd PluginCommand) error
    RegisterSlot(name string, pv PluginView)
    Slots() map[string][]PluginView
}
```

插件视图在**主线程**上通过宿主提供的构建时机注入，宿主负责布局和重绘调度。插件不需要（也不能）直接操作窗口。

### 6.3 插件动态加载的现实约束与方案

**静态注册的宿主内插件**（编译进主二进制）没有运行时加载问题，适合第一批核心插件（编排树、时间线、Diff 审阅）。

**第三方动态插件**必须走**子进程 + IPC**：插件作为独立进程运行，通过 JSON-RPC/gRPC 与宿主通信。对于 UI，有两种子模式：

- **UI 描述子进程**：插件进程返回结构化的 UI 描述（类似声明式 JSON），宿主在 MyGo 中统一渲染。安全、跨语言，但表达力受宿主渲染器限制。
- **独立窗口子进程**：插件进程自己创建 MyGo 窗口（或 WebView），宿主只负责启动和生命周期管理。灵活但隔离边界较弱。

**不推荐** `plugin` 包：平台受限、不可卸载、版本强耦合，不适合插件生态。


## 七、目录结构与依赖

```
locowrk/
├── cmd/locowrk/
├── internal/
│   ├── kernel/              # sky-valley/pi 封装
│   ├── eventbus/            # Event 定义与 Bus
│   ├── adapter/             # OrchestrationAdapter + 兼容矩阵
│   ├── skills/
│   └── bridge/              # HarnessService（MyGo Bind）
├── desktop/
│   ├── plugin.go            # DesktopPlugin / PluginHost 接口
│   ├── host.go              # 插槽管理、构建调度
│   ├── orchestration-tree/  # 原生 ui.View 插件
│   ├── workflow-timeline/   # 原生 ui.View 插件
│   ├── tool-inspector/      # 原生 ui.View 插件
│   └── diff-review/         # WebView 插件
├── proto/                   # 子进程插件 IPC 协议
└── go.mod
```

关键依赖：`github.com/sky-valley/pi`、`github.com/egoist/mygo`、`google.golang.org/grpc`（子进程 IPC，可选）。


## 八、分阶段落地路线图

### M0 — 内核接入与事件流（1–2 周）

跑通 `sky-valley/pi` Agent Loop，事件映射到 EventBus，stdout 验证。**验收**：一次完整工具调用产生正确事件序列；JSONL 会话可持久化。

### M1 — 插件适配器骨架（2–3 周）

实现 `OrchestrationAdapter`，以子进程启动 mock 编排插件，订阅编排事件，**拒绝所有 UI 调用**并记录过滤日志。**验收**：mock 事件正确转发；`ctx.ui.overlay` 返回 `ErrUIUnsupported`，主流程不中断。

### M2 — MyGo 最小宿主与原生事件可视化（2–3 周）

创建 MyGo 窗口，`HarnessService` 通过 Typed IPC 暴露 `SubscribeEvents`，前端用虚拟化列表渲染消息流和工具调用时间线。**验收**：桌面窗口实时显示 Agent 输出；输入框可发送消息；窗口空闲时 CPU 接近 0%（MyGo 的零轮询模型）。

### M3 — PluginHost 与首批原生 UI 插件（3–4 周）

定义 `DesktopPlugin` / `PluginHost` 接口，实现 `OrchestrationTreePlugin` 和 `WorkflowTimelinePlugin`，用 MyGo 原生 `ui.View` 绘制。**验收**：编排 mock 触发时，桌面端实时更新子代理树和 workflow 阶段条；插件可独立启用/禁用；**验证 `ui.Element` 不出现在插件状态或后台 goroutine 中**。

### M4 — WebView 混合渲染与富交互插件（2–3 周）

实现 `DiffReviewPlugin`，在原生窗口内嵌 `ui.WebView`，Go 侧执行 `git diff`，前端渲染高亮差异。**验收**：Diff 面板与原生控件并列显示；`ui.WebView` 在视图外创建一次；切换 tab 时 WebView 页面状态保持。

### M5 — 真实 Pi 编排插件接入与技能复用（3–4 周）

把 `pi-orchestrator` 通过 TS→JSON-RPC 桥接接入适配器，验证 single/parallel/chain/async 委派；加载 `~/.pi/agent/skills`。**验收**：真实编排插件的 parallel 委派在桌面端可见；steering 可从桌面发回；TUI 命令被静默丢弃，桌面端用原生插件展示等价信息。

### M6 — 子进程动态桌面插件（探索性，2–3 周）

定义一个 `mygo-plugin.json` 风格的插件 manifest（可参考 MyGo 对原生动态库的声明方式），实现“子进程返回 UI 描述 → 宿主渲染”的第三方插件通路。**验收**：一个最小第三方插件（独立进程）能在宿主中挂载一个可视化面板，宿主进程不因插件崩溃而退出。


## 九、关键风险与应对

**风险一：`ui.Element` 生命周期误用**。插件作者可能把 Element 存入状态或后台 goroutine，导致跨帧崩溃或竞态。应对：`PluginHost` 的契约文档强制规定“插件只持有数据状态”；在开发构建中加入 `ui.Tester` 的帧生命周期断言测试。

**风险二：WebView 与原生 UI 的层级/焦点冲突**。WebView 覆盖区域内的弹出层需要特殊处理。应对：利用 MyGo 的覆盖层机制（Popover、Dialog、Tooltip 画在网页之上），插件契约中明确“WebView 内的模态由宿主覆盖层统一管理”。

**风险三：子进程插件的生命周期与崩溃**。应对：`exec.CommandContext` + 心跳超时 + 自动重启；gRPC 方案可用 `go-plugin` 自带的进程监督和 AutoMTLS。所有 IPC 调用加超时、重试与熔断。

**风险四：桌面原生插件与编排插件的语义漂移**。所有桌面插件只消费 EventBus 的结构化事件，不直接读取编排插件内部状态；事件契约版本化维护。

**风险五：MyGo 成熟度**。M0/M1 阶段不依赖 MyGo，用 CLI/stdout 验证内核和适配器；M2 引入 MyGo 后，`HarnessService` 接口保持与 UI 框架无关，必要时可替换宿主而不改内核。


## 十、设计总结

这个方案的核心思想可以概括为：**内核用 Go 移植，编排插件用适配器“借”逻辑，桌面插件在 MyGo 上“长”界面，TUI 调用在适配器边界被干净地挡住。** 桌面插件画界面有两条一等公民路径——纯 Go 原生 `ui.View`（GPU 直接绘制，适合结构化可视化）和系统 WebView（适合富交互面板）——宿主通过自实现的 `PluginHost` 插槽机制将两者统一挂载。插件动态加载不碰 Go `plugin` 包的雷区，走子进程 + IPC 的进程隔离路线。每一步都有可运行的验收物，而不是先设计一个庞大的插件平台再填内容。