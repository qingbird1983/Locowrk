# 调研 02：pi 的扩展面与本项目的兼容边界

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0003；architecture/04

> 依据：立项方案 v1.1、`sky-valley/pi` 的 UPSTREAM 范围裁定、pi 公开文档。**事件名与 API 签名已于 2026-10-28 对照上游源码复核**（`earendil-works/pi` 提交 `38ff9917`，见 §4）；上游源码不随本仓库抓取。本文锁定“分类与边界”。

## 1. pi 的四层扩展体系

| 层 | 形态 | 与界面的关系 | 本项目策略 |
|---|---|---|---|
| **Extensions** | TypeScript 模块，运行在宿主提供的 API 上 | 混合：工具/命令/钩子与 UI 无关；`ctx.ui.*`、快捷键是终端专属 | 选择性兼容（A 轨） |
| **Skills** | `SKILL.md` + 可选脚本/资源 | 无关 | 直接复用（上游已移植） |
| **Prompt Templates** | Markdown 模板 | 无关 | 上游未移植；本项目暂不支持 |
| **Themes** | 终端颜色 token | 终端专属 | 丢弃；桌面视觉由 MyGo/宿主主题掌控 |

## 2. Extensions 的能力面（按桌面兼容性分类）

| 能力 | 典型 API | 桌面处理 | 理由 |
|---|---|---|---|
| 注册工具 | `registerTool(...)` | 兼容 | 输入/输出是结构化数据，与 UI 正交 |
| 注册命令 | `registerCommand(...)` | 兼容（映射为桌面命令面板/斜杠命令） | 命令是行为，不是渲染 |
| 事件钩子 | `on(...)` | 兼容（按事件白名单） | 可观察/拦截 Agent 行为 |
| 修改提示词/上下文 | 钩子 + 会话 API | 兼容（需审计） | 与模型交互有关 |
| 子代理 / workflow / 异步编排 | 扩展自行实现或调用宿主 | 兼容（A 轨核心价值） | 编排逻辑与终端无关 |
| `ctx.ui.notify` / `select` / `confirm` / `input` | 基础对话框 | **支持**：映射到宿主通知/对话框 | 通用交互语义，桌面有等价物 |
| `ctx.ui.overlay` / `setWidget` / `setFooter` / `setHeader` / `setTitle` / 编辑器操控 | 自定义终端 UI | **no-op + 诊断** | 终端组件在桌面无意义；抛错会中断扩展 |
| 快捷键 | `registerShortcut(...)` | **no-op + 诊断** | 快捷键归宿主；插件不得抢占 |
| provider / 虚拟模型 / MCP 注册 / session 读写 | 宿主扩展面 | **缓做**：返回“未实现”诊断 | 需要宿主能力，后续版本转正 |
| 自定义渲染器 / 工具 `renderCall`/`renderResult` | TUI renderer | **no-op + 诊断** | 桌面用原生插件重建等价信息 |
| 主题 | 终端颜色 | **丢弃** | 桌面主题体系不同 |

## 3. 为什么 TUI 成员 no-op，而不是抛错

> 2026-10-28 修订：按 ADR-0003 改为“no-op + 诊断”。此前写的是返回 `ErrUIUnsupported`；理由见下。

- 扩展代码通常不写 `try/catch`；抛错会把“界面上用不了”升级成“整个插件不能用”。
- no-op 不等于静默：每个扩展每个成员记一条结构化诊断，UI 有“兼容性诊断”面板，用户能看见缺失。
- 安全能力（文件写删、执行、网络）仍走能力闸，未声明就拒绝；这与兼容降级是两回事，必须分开记录。
- 基础对话框（notify/select/confirm/input）有桌面等价物，映射为宿主实现，不降级。
- 约定：缓做成员返回零值 + `not_implemented`；不支持成员返回零值 + `unsupported`；两者都不 throw。

## 4. 上游源码复核（2026-10-28，提交 `38ff9917`）

对照文件：`packages/coding-agent/src/core/extensions/types.ts`、`docs/extensions.md`、`examples/extensions/**`。源码只读、不入库。

### 4.1 模块、导出与初始化

- 扩展默认导出工厂：`export default function (pi: ExtensionAPI)`，可同步或异步；宿主等待异步工厂完成后继续。
- 类型与工具来自 `@earendil-works/pi-coding-agent`；示例还会用 `@earendil-works/pi-ai`（`StringEnum`）、`@earendil-works/pi-tui`（`Text`/`Container`/`Markdown`/`matchesKey`/`truncateToWidth`）、`typebox`（`Type`）。
- 加载方式：单文件 `.ts/.js` 或目录（`index.ts`/`index.js`），上游用 jiti 直接执行 TS；pkg 依赖放在旁边的 `package.json`。
- 生命周期：工厂 →（必要时 `project_trust`）→ `session_start` → … → `session_shutdown`；reload 会替换运行时，旧上下文失效。工厂里禁止起长期进程/定时器，应在 `session_start` 起、`session_shutdown` 收。
- 扩展与 pi 同进程、同 OS 权限；上游文档明确“只加载可信来源”。

### 4.2 事件（41 个，按用途分组）

| 组 | 事件 |
|---|---|
| 会话 | `project_trust`、`session_start`、`session_info_changed`、`session_before_switch`、`session_before_fork`、`session_before_compact`、`session_compact`、`session_compact_failed`、`session_shutdown`、`session_before_tree`、`session_tree`、`mcp_servers_change`、`resources_discover` |
| 运行/轮次 | `input`、`before_agent_start`、`agent_start`、`agent_end`、`agent_before_settle`、`agent_settled`、`turn_start`、`turn_end`、`cache_warming_decision` |
| 模型请求 | `context`、`context_with_system`、`before_provider_headers`、`before_provider_request`、`after_provider_response`、`provider_stream_event`、`model_select`、`thinking_level_select` |
| 消息 | `message_start`、`message_update`、`message_end`、`ui_prompt_start`、`ui_prompt_end` |
| 工具 | `tool_execution_start`、`tool_execution_update`、`tool_execution_end`、`tool_call`、`tool_result`、`user_bash` |

- 处理器按注册顺序执行；`pi.on()` 返回取消订阅函数。
- 四类组合语义：通知（返回值忽略）、就地修改、链式转换（`input`/`context`/`context_with_system`/`message_end`/`tool_result`/`before_provider_request`）、闸门（`tool_call` 返回 `{block}`、`input` 被处理即停止回退）。
- `tool_call` 处理器抛错会**阻断该工具**（fail-safe）；其他处理器出错只记录并继续。
- `agent_end` 不是最终空闲信号；`agent_before_settle` 可请求一次继续，`agent_settled` 才是通知用的终态。

### 4.3 工具注册

- `pi.registerTool({ name, label, description, parameters, execute, renderCall?, renderResult?, ... })`。
- `parameters` 是 TypeBox schema；`execute(toolCallId, params, signal, onUpdate, ctx)` 返回 `{ content, details }`，抛出即失败结果。
- 可选：`outputSchema`/`structuredContent`、`exposure`（direct/model-only/codemode/deferred/hidden）、`namespace`、`annotations`、`executionMode`、`prepareLoadout`、`reachesTools`、`renderShell`。
- 工具渲染器（`renderCall`/`renderResult`、`registerToolRenderer`）是 TUI 面；桌面按 no-op + 诊断处理。

### 4.4 `ctx.ui` 方法集与返回值

| 方法 | 返回值 | 本项目 |
|---|---|---|
| `select(title, options, opts?)` | `Promise<string | undefined>` | 映射宿主选择框 |
| `confirm(title, message, opts?)` | `Promise<boolean>` | 映射宿主确认框 |
| `input(title, placeholder?, opts?)` | `Promise<string | undefined>` | 映射宿主输入框 |
| `editor(title, prefill?)` | `Promise<string | undefined>` | 映射多行编辑框 |
| `notify(message, type?)` | `void` | 映射宿主通知 |
| `setStatus(key, text?)`、`setWidget(key, content?, opts?)`、`setFooter`、`setHeader`、`setTitle`、`custom`、`setEditorText`、`pasteToEditor`、`setEditorComponent`、`addAutocompleteProvider` | void / Promise（`custom`） | TUI no-op + 诊断；`custom` 返回 `undefined`，扩展继续 |
| `theme`、`getTheme(s)`、`setTheme`、`getToolsExpanded`、`setToolsExpanded`、`onTerminalInput` | 混合 | 主题与展开状态缓做；终端输入监听 no-op |
| `getEditorText`、`setWorkingMessage/Visible/Indicator`、`setHiddenThinkingLabel` | 混合 | 缓做或 no-op，按矩阵 |

- 模式：`"tui" | "rpc" | "json" | "print"`；`ctx.hasUI` 在 tui/rpc 为真。终端专属行为应以 `ctx.mode === "tui"` 守卫——桌面模拟为 `"rpc"`，这样扩展自带降级路径。
- 上下文还提供 `sessionManager`（只读，`getBranch`/`getLeafEntry`/`getEntries`）、`modelRegistry`、`model`、`thinkingLevel`、`signal`、`abort`、`isIdle`、`isProjectTrusted`、`shutdown`、`getContextUsage`、`compact`、`getSystemPrompt`。命令上下文另有 `waitForIdle`/`newSession`/`fork`/`navigateTree`/`switchSession`/`reload`。

### 4.5 加载顺序、隔离与相互调用

- 加载位置：用户目录、项目目录（需信任）、设置里的附加路径、命令行 `--extension`；单文件或目录入口。
- 失败隔离由 pi 负责“记录并继续”；本项目在子进程边界再做一层（崩溃不拖垮宿主）。
- 扩展之间用 `pi.events`；扩展调用其它工具用 `ctx.executeTool(name, args, { signal, onUpdate })`；给会话发消息用 `pi.sendMessage`/`sendUserMessage`，持久数据用 `pi.appendEntry`。
- 子代理示例不依赖宿主 API 完成委派，而是自己 `spawn` 一个 `pi` 进程并用 JSON 模式读事件——这正是 A 轨必须能跑 Node 内置模块的原因（ADR-0009）。

## 5. 对本项目的直接结论

- **不要把“兼容 pi 插件”当宣传语**；准确说法是“兼容编排型扩展，终端界面能力降级为 no-op 并给出诊断”。
- A 轨适配器必须有**三分类矩阵**与**诊断面板**，这是产品的一部分，不是调试附件。
- B 轨桌面插件不是“翻译 TUI”，而是围绕事件流重新设计的信息呈现；两者不要混为一谈。
- Skills 是当前性价比最高的复用面：上游已实现发现、校验与注入，我们只需做信任 UI 与展示。
