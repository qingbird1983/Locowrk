# 设计：Phase 6 A 轨运行时、打包与 shim

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0009、ADR-0015、ADR-0004；research/02、03；design/phase-2/runtime-spike.md；[extensions.md](extensions.md)

## 1. 目标与非目标

**目标**：把真实 pi 编排扩展作为预打包的 Node 子进程运行；入口、shim、能力清单可复现；运行时随包分发且可审计；扩展包不依赖用户环境。

**不做**：运行期 `npm install`、远程加载代码、直接加载 `~/.pi` 的 TypeScript、原生 `.node` 模块、SEA 单文件、自动更新运行时。

## 2. 目录与发布形态

```text
runtime/
  node/                 发布物；不入 git
    node.exe            Windows（macOS/Linux 为 bin/node）
    LICENSE
    README.md           版本、来源 URL、SHA-256
extensions/             用户扩展目录（%AppData%\Locowrk\extensions）
tools/extension-build/  开发期打包工具（package.json + lock 入库）
fixtures/extensions/    自写编排样本源码（入库）
```

- 生产只认 `runtime/node/`，由 `cmd/locowrk` 解析为绝对路径传给 `adapter.Options.NodePath`；**发布构建缺运行时 fail-closed**。
- 开发期允许回退 PATH 的 `node` 并打印警告；测试/冒烟用 `LOCOWRK_NODE` 覆盖，不写设置。
- 版本钉死在 `runtime/README.md`（Phase 2 spike 用 Node v24.14.0；实现时以发布包为准）与 `tools/extension-build/package.json` 的 engines 中记录。

## 3. 扩展包格式

一个扩展包是一个目录：

```text
<name>/
  plugin.json     适配器清单（name/version/protocol/entrypoint/capabilities）
  bundle.mjs      由构建工具生成的单文件入口
```

- `entrypoint` 固定为 `bundle.mjs`；`protocol` 为 1；`capabilities` 默认空数组（缺失 = 无能力）。
- 清单只描述适配器启动与权限；扩展名称、来源、构建时间写入 `build.json`（不参与协议，供 UI 显示）。
- 不把扩展源码、`node_modules`、`.ts` 放进发布包；安装包只有上述文件。

## 4. 构建流程（开发期）

```text
extension source/
  index.ts          export default function (pi: ExtensionAPI) { ... }
  extension.json    {name, version, capabilities, source?}
        │
        ▼  tools/extension-build/build.mjs
plugin.json + bundle.mjs
```

- 构建工具用已准入的 esbuild（版本锁定）；入口包装：
  1. 读取 `extension.json`；
  2. 生成临时入口 `import factory from "<source>"; import { run } from "@locowrk/shim"; run(factory, {...})`；
  3. esbuild 打包为 ESM 单文件；`node:*` 保持 external；`typebox` 等纯 JS 依赖打进包；
  4. 写 `plugin.json` 与 `build.json`。
- 拒绝：原生 `.node` 模块、动态 `import()` 远程 URL、构建期网络访问、未在 `extension.json` 声明的能力。
- 构建可复现：源码路径、esbuild 版本、Node engines、输出 SHA-256 写 `build.json`；同样输入产生同样字节（不含时间戳）。
- **运行期不构建**：用户拿到的是预打包目录；开发者的构建命令只用于开发/验收。

## 5. shim（`locowrk-shim/1`）

shim 是一个随每个 `bundle.mjs` 打包的 TypeScript 入口，负责：

1. 在 stdin/stdout 上说 JSONL JSON-RPC；
2. 等待宿主 `initialize`，发送 `initialize` 响应（含 `shim` 版本与声明的能力交集）；
3. 调用扩展默认工厂，提供 `pi`/`ctx` 对象；
4. 把 `registerTool`/`registerCommand` 转成宿主请求；
5. 把宿主的 `event` 分发给 `pi.on` 注册的处理器；
6. 把宿主 `invoke_tool`/`invoke_command` 转成扩展回调；
7. 收到 `shutdown_plugin` 后调用扩展的 `session_shutdown` 处理器并退出 0。

**版本**：`initialize` 结果增加可选 `shim: "locowrk-shim/1"`；宿主不兼容时禁用扩展并记 `incompatible_shim` 诊断（不改 `protocol`，属新增可选字段）。

**API 子集（Phase 6 最终矩阵）**

| 成员 | 处理 |
|---|---|
| `registerTool`（含 `execute`、`parameters`） | 支持；工具进入内核，走审阅/取消 |
| `registerCommand` | 支持；桌面命令面板列出并执行 |
| `on(...)`：生命周期/消息/工具/编排事件的通知型钩子 | 支持；按固定 41 事件白名单订阅 |
| `on("tool_call")` 返回 `{block}` | 支持；宿主在工具执行前等待插件回执 |
| `on("input")`/`context`/`context_with_system`/`before_provider_request` 的链式改写 | 缓做：返回 `not_implemented` 诊断，原值继续 |
| `ctx.ui.notify` | 支持；宿主通知区 |
| `ctx.ui.select/confirm/input/editor` | 支持；宿主原生对话框，Promise 等待用户 |
| `ctx.ui.setWidget/setFooter/setHeader/setTitle/setStatus/custom/setEditorText/pasteToEditor/setEditorComponent/addAutocompleteProvider` | TUI no-op；`custom` 返回 `undefined`；每成员一条诊断 |
| `ctx.sessionManager` 只读、`ctx.model`、`ctx.thinkingLevel`、`ctx.signal`、`ctx.isIdle` | 支持（只读 DTO） |
| `ctx.executeTool`、`pi.sendMessage`/`sendUserMessage`、`appendEntry`、`ctx.compact`、`ctx.shutdown` | 缓做/不支持，按矩阵返回诊断 |
| `registerShortcut` | no-op + 诊断 |
| `spawn_subagent` 宿主调用 | 缓做；子代理由扩展自行 spawn，事件经 `emit_event` 上报 |

- 所有缓做/不支持成员**永不 throw**；诊断去重键仍为 `plugin + member + category + code`。
- `tool_call` 的闸门回执需要请求/响应：宿主把 `event` 作为请求发送（`wantsEventAck`），插件回 `event_result {id, result}`；Phase 6 新增此可选方法，其他事件仍可通知。
- 工具执行进度：新增插件→宿主 `tool_update {toolCallId,text}`，映射为 `tool_execution_update`；`invoke_tool` 最终结果仍同步返回。

## 6. 运行时生命周期

- 每个已启用、已信任的扩展一个 Node 子进程；一个进程服务当前工作区的所有会话（Phase 6 桌面只有一个活动会话）。
- 进程在应用启动后、首次需要工具前启动；`initialize` 的 `session.id` 允许为空，随后由宿主 `session_start` 事件带入。
- 切换会话不重启进程；宿主发送 `session_shutdown`（旧）与 `session_start`（新）事件，并把后续 `invoke_tool` 的 `sessionId` 换成新值。
- 工作区/扩展启停变化时停止并重启对应进程；崩溃按适配器现有重启策略，超过上限禁用。
- 关闭应用：先发 `shutdown_plugin`，再按适配器超时杀进程树。

## 7. 真实样本与验收

| 样本 | 用途 | 来源 |
|---|---|---|
| `todo.ts` | 真实 pi 扩展：工具注册、`session_start`、`notify`、`setStatus` no-op | 上游固定提交，构建时外部检出，不入库 |
| `rpc-demo.ts` | 真实 pi 扩展：命令、`setWidget/setTitle` no-op | 同上 |
| `fixtures/extensions/orchestrator-sample` | 并行/链式/异步子代理、workflow、取消、进度 | 本项目自写，入库 |
| `fixtures/plugins/mock-orchestrator` | 协议与故障注入 | 已有 |

- 上游样本按 MIT 使用，仓库只记录提交与构建命令，不复制源码；`build.json` 记录来源。
- 验收至少跑 `todo` + `orchestrator-sample`；`rpc-demo` 用于确认 UI no-op 诊断；`subagent` 上游样本仅手动加载，不纳入自动门禁（需 pi 二进制与特权）。

## 8. 安全与失败

- 扩展是**受信任代码**；manifest 能力是契约与审计，不是沙箱。安装/首次信任界面必须明示。
- `NodePath` 缺失、版本不符、`plugin.json` 非法、`bundle.mjs` 缺失或哈希不符：禁用并诊断，不启动。
- 单行 1 MiB、输出速率、进程树清理按 `proto/plugin-protocol.md`；Phase 6 增加大工具输出与孙进程测试。
- 扩展不能读取宿主凭证；环境变量只透传白名单（`PATH`、代理、locale），不传 key。

## 9. 测试

- 构建：同一源码两次构建字节一致；`plugin.json` 校验；拒绝原生模块/远程 import。
- shim：协议 fixture 覆盖 initialize/register/event/invoke/command/ui/关停；每个矩阵成员“返回值 + 诊断 + 不抛错”。
- 运行时：缺 Node、版本不符、入口缺失、哈希不符、崩溃/超时/进程树；NodePath 与 PATH 回退。
- 真实样本：`todo` 工具与命令可用；`rpc-demo` no-op 诊断；`orchestrator-sample` 事件顺序与取消。
- 平台：Windows 必测；macOS/Linux 构建通过；Node 目录按平台。

## 10. 风险与待决

- **Node 目录体积/签名**：发布工程 backlog；ADR-0015 已定目录，SEA 不在本阶段。
- **shim 覆盖面**：链式改写缓做会让部分扩展行为不完整，必须有可见诊断，不能静默。
- **`tool_call` 闸门**：需要请求/响应与超时；插件无响应时按“不阻断 + 诊断”还是 fail-safe 阻断，需在实现前定（倾向 fail-open 并诊断，避免死锁）。
- **上游语义变化**：shim 表以研究文档固定提交为蓝本；升级独立评估。
