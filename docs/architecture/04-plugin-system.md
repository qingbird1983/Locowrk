# 架构 04：插件系统

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004、ADR-0008、ADR-0009；architecture/01、03；research/02

## 1. 两条轨道

| | A 轨：编排兼容 | B 轨：桌面原生 |
|---|---|---|
| 来源 | pi 的 TypeScript 扩展（或按同一语义重写的 Go 实现） | 本项目定义的 Go 插件 / 第三方子进程插件 |
| 关心什么 | 工具、命令、事件钩子、子代理、workflow、异步编排 | 界面、面板、插槽、交互 |
| 运行位置 | 子进程（进程隔离） | 宿主内（静态注册）或子进程（动态） |
| 界面 | **不提供终端渲染**；TUI 成员 no-op + 诊断，基础对话框映射宿主 | 原生 `ui.View` 或系统 WebView |
| 状态 | 兼容降级与安全拒绝分开诊断 | 插件只持有数据状态 |

**不做的事**：不翻译终端 UI、不模拟 TUI 组件、不运行任意 Node 包、不用 Go `plugin` 包。

## 2. A 轨：编排适配器

### 2.1 现状与缺口

仓库内内核（fork 自 `sky-valley/pi`）**没有移植** pi 的 extensions runtime（上游 `core/extensions/**` 被其范围测试判为宿主面）。因此 A 轨不是“打开开关”，而是本项目要补的一层：

- **桥接模式（已选定）**：适配器启动一个 Node 子进程，加载安装期打包好的扩展产物（esbuild 单文件），注入显式子集 shim（`locowrk-shim/0`），把注册/事件/工具调用翻译成 JSON-RPC。选型与读数见 ADR-0009 与 [`design/phase-2/runtime-spike.md`](../design/phase-2/runtime-spike.md)。
- **重写模式（不采用）**：按扩展语义用 Go 重写；spike 证明它只能逐个扩展重写，兼容面退化为“编排语义”。仅作为 B 不可行时的兜底。

spike 实测（Windows 11）：冷启动 55~63ms、常驻约 49MiB、上游 rpc-demo/todo 可运行、subagent 可加载；QuickJS 路线更快更省，但编排样本依赖 Node 内置模块，执行失败。Node 运行时随产品分发并锁版本，运行期不执行 `npm install`、不远程加载代码。

### 2.2 接口

```go
type OrchestrationAdapter interface {
    Load(ctx context.Context, path string) (*Manifest, error)
    Start(ctx context.Context, m *Manifest) error
    Events() <-chan eventbus.Event
    SendCommand(ctx context.Context, cmd Command) error
    Tools() []ToolDefinition
    Stop(ctx context.Context) error
}

type Capability struct {
    Name  string   // "fs.read" | "fs.write" | "fs.delete" | "exec" | "net"
    Scope []string // 路径 glob 或域名；空表示无范围（fail-closed）
}

type Manifest struct {
    Name         string
    Version      string
    Protocol     int      // 子进程协议版本
    Capabilities []Capability
    Entrypoint   string
}
```

### 2.3 兼容矩阵（A 轨契约）

| 分类 | 成员 | 行为 |
|---|---|---|
| **支持** | `registerTool`、`registerCommand`、事件钩子（agent/turn/message/tool/context/system prompt）、工具开关、模型/思考档、子代理/workflow/异步、`ctx.ui.notify/select/confirm/input` | 真实实现；基础对话框映射到宿主 UI |
| **缓做** | provider/虚拟模型/MCP 注册、session 读写/切换/新建/分叉、`registerFlag`、autocomplete、markdown transformer、模型目录写操作 | 成员存在，返回“未实现”诊断，**不抛错** |
| **不支持** | widget/footer/header/title、`ctx.ui.custom`、编辑器操控、自定义 renderer、工具 `renderCall`/`renderResult`、`navigateTree`、快捷键、`shutdown` | 成员存在但 no-op，**永不 throw**；每扩展每成员一条诊断 |

- 安全能力（文件写删、执行、网络）独立走能力闸：未声明 = 拒绝。兼容降级不得绕开安全拒绝。
- **Node 原生能力的边界**：B 路线下扩展可直接 `require("node:fs")` 等，shim 拦不住；manifest 能力声明是契约与审计依据，未加 OS 级隔离前，A 轨扩展只能按“受信任代码”处理（ADR-0009、standards/06）。
- 清单是契约；上游没有 semver，增减走 ADR/文档修订。
- 事件名与签名已按固定上游提交 `38ff9917` 复核（research/02 §4）；扩表前先改该文与本节。

### 2.4 诊断语义

- 结构：`{plugin, member, category, code, message, firstAt, count}`。
- 去重：同一插件同一成员只记一条，`count` 累加；UI 提供“兼容性诊断”面板。
- 禁止：静默 no-op（必须留诊断）；用异常表达“不支持”（会中断扩展）。
- 缓做成员返回零值 + `not_implemented` 诊断；不支持成员返回零值 + `unsupported` 诊断。
- 安全拒绝单独记录，不能混进“兼容性诊断”，否则用户分不清“功能缺失”与“被安全策略拦住”。

### 2.5 命令与事件

- 宿主 → 插件：`prompt`、`steer`、`follow_up`、`abort`、`tool_result`、`invoke_tool`、`event`、`shutdown_plugin`。
- 插件 → 宿主：`register_tool`、`register_command`、`emit_event`、`spawn_subagent`、`workflow_step`、`diagnostic`、`ui_notify`、`ui_set_widget`、`fs_write`。
- 所有消息带 `id`、`method`、`params`；错误用 JSON-RPC error 对象，`code` 区分 `unsupported`、`invalid_params`、`timeout`、`crashed`。
- `shutdown_plugin` 是宿主生命周期命令；扩展 API 的 `ctx.shutdown` 属 TUI 面，按矩阵 no-op，两者不要混用。

## 3. B 轨：桌面插件宿主

### 3.1 接口

```go
type DesktopPlugin interface {
    Name() string
    InterestedEvents() []eventbus.Kind
    // 原生路径：返回视图；WebView 路径返回 nil
    View(ctx *ui.Context, host PluginHost) ui.View
    WebViewFactory() (WebViewSpec, bool)
    OnEvent(eventbus.Event)
}

type PluginHost interface {
    Subscribe(kinds []eventbus.Kind) <-chan eventbus.Event
    Dispatch(ctx context.Context, cmd Command) error
    RegisterSlot(name string, pv PluginView)
    Slots() map[string][]PluginView
}
```

`PluginHost` 是宿主层自己实现的抽象；MyGo 没有“插件插槽”原语。宿主维护 `map[slot][]PluginView`，布局时按 slot 名称嵌入。

### 3.2 插槽

首批插槽（Phase 4）：

| 插槽 | 位置 | 典型插件 |
|---|---|---|
| `session.sidebar` | 会话页右侧栏 | 编排树、子代理状态 |
| `session.timeline.above` | 消息流上方 | workflow 阶段条 |
| `session.tool.detail` | 工具卡片展开区 | 工具检查器 |
| `session.tab` | 主区域标签页 | Diff 审阅、文件预览 |
| `settings.section` | 设置页 | 插件设置 |

插槽是布局契约，不是权限；插件能否出现由 manifest/注册表与用户启停决定。

### 3.3 原生 `ui.View` 规则

1. 视图是**应用状态的函数**，每帧重建，不缓存 Element。
2. `ui.Element` 在下一帧前失效；不得存入结构体字段、包级变量、channel 或后台 goroutine。
3. 持久状态（焦点、滚动、输入中文本）靠元素在兄弟中的位置或 `Key` 保持，不靠保存 Element。
4. 回调捕获 `Services`（长期有效），不捕获 `Context`（帧级）。
5. 每帧构建必须快：大列表用虚拟化；重计算放后台，结果写回数据状态后 `Invalidate`。

### 3.4 WebView 规则

- WebView 在视图外创建一次（窗口初始化或插件启用时），视图只负责摆放与显隐。
- 前端通过 MyGo 生成的 TypeScript 客户端调用 Go；事件用 Typed IPC channel，不用轮询。
- 弹层（菜单、对话框、tooltip、toast）由宿主覆盖层统一管理；WebView 内不实现会与原生层冲突的模态。
- 页面导航白名单：只允许本地资源与显式声明的 origin；外部链接交系统浏览器。

## 4. 子进程插件（动态加载）

Go 的 `plugin` 包不可用于生产：仅 Linux/macOS、不能卸载、主程序与插件必须完全同版本同编译标志。第三方插件一律走子进程 + IPC。

两种 UI 子模式：

| 模式 | 做法 | 适用 |
|---|---|---|
| UI 描述 | 插件返回声明式 JSON，宿主统一渲染 | 跨语言、安全、表达力受宿主限制 |
| 自有窗口 | 插件进程自己开 MyGo 窗口或 WebView，宿主只管生命周期 | 复杂界面、独立窗口；隔离边界较弱 |

首版（Phase 7）只做 UI 描述模式；自有窗口模式留 backlog。

## 5. 生命周期与故障

```
discovered → validated → started → ready → stopped
                    ↘ failed（诊断可见，可重试）
```

- 启动：校验 manifest 与协议版本 → 分配能力 → `exec.CommandContext` → 握手（10s）→ 注册。
- 健康：心跳 5s，连续 3 次失败视为失联；单次 RPC 默认 30s，长任务显式声明。
- 崩溃：记录退出码与最后 100 行 stderr（脱敏）→ 按策略重启（指数退避，最多 3 次）→ 仍失败则禁用并提示。
- 退出：先发 `shutdown`，5s 后取消 context，再 2s 后 kill；清理子进程树。
- 资源：单插件内存上限（默认 512 MiB）、CPU 时间预算、输出速率限制；超限先告警再终止。

## 6. 安全与信任

- **默认无能力**：文件、网络、执行、环境变量都要 manifest 声明 + 用户逐项批准；权限采用“权限 + 范围”（如 `fs.read` + 路径 glob、`net` + 域名白名单），缺失范围 = 无权限；全树写/删直接判非法。
- **硬拒绝**：`.env*`、SSH 凭证、`.git/**` 的读写；全树写删；未声明的网络域。删除只走系统回收站、非递归，并做速率限制。
- **信任级别**：A 轨编排扩展与 Agent 同权（能注册工具/钩子），按“受信任代码”处理；进程隔离不是安全沙箱。B 路线保留 Node 原生 fs/net/exec，能力闸无法拦截绕过 shim 的原生调用，必须在安装与运行界面明示“仅受信任插件”。
- **审计**：每次授权、拒绝、崩溃、重启都留可查记录；记录脱敏。
- **签名**：动态插件分发包后续加签名校验；首版只支持本地开发目录 + 显式信任。
- **项目资源**：项目内技能/插件默认不加载，需用户信任该工作区（接缝 #11）。

## 7. 非目标（明确不做）

- 不在桌面端渲染 pi 的终端组件或 ANSI 界面。
- 不提供“任意 npm 包即插件”的兼容层。
- 不允许插件直接操作窗口、菜单、快捷键（这些是宿主 API，须经授权与封装）。
- 不让插件把未脱敏凭证或原始会话文件带出进程。
