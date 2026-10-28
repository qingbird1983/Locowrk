# 设计：Phase 3 WebView 前端与 Typed IPC

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0011；[shell.md](shell.md)；standards/04 §6、08 §5

## 1. 技术选择

| 项 | 选择 | 理由 |
|---|---|---|
| 渲染 | MyGo WebView（系统 WebView2 / WKWebView / WebKitGTK） | ADR-0011 |
| 语言 | TypeScript（严格模式） | 与生成式客户端一致，避免 JS 漂移 |
| 构建 | esbuild（npm 脚本） | 单命令、无框架、产物小；不引入第二套 UI 框架 |
| 客户端 | `mygo generate` 生成的 `mygo.ts` | 不手写第二套 IPC（standards/04 §6） |
| 资源服务 | `mygo.SetFrontend(embed.FS)` | 单二进制，运行期不读磁盘、不连网 |

前端只做“显示与输入”，不持有业务状态：会话、消息、模型、凭证状态都以 Go 为准。

## 2. 目录与构建

```
web/
  package.json          esbuild + mygo-runtime（锁文件入库）
  tsconfig.json
  src/main.ts           界面装配：会话、输入、状态行、事件分发
  src/transcript.ts     消息模型 + 窗口化列表（只挂载视口附近）
  src/smoke.ts          验收场景脚本（仅 ?smoke= 时被调用）
  src/style.css
  src/mygo.ts           生成物，不手改（提交入库，便于离线构建）
  index.html
  dist/                 构建产物（提交入库；单文件 <1MB，合计 <10MB）
```

- `npm install`：开发期一次性；`node_modules/` 不入库。
- `npm run build`：`esbuild src/main.ts --bundle --format=esm --target=es2020 --outfile=dist/app.js` + 复制 `index.html`、`style.css`。
- 生成客户端：`go run ./cmd/locowrk -generate-bindings web/src/mygo.ts`。该模式只注册绑定、调用 `mygo.WriteTypeScript`，不开窗口、不跑会话。
- `desktop` 用 `//go:embed all:web/dist`；`go build` 不依赖 npm。
- 可复现检查（阶段门禁）：重新生成客户端与构建后 `git diff --exit-code web/`。

## 3. IPC 契约

唯一事实源是 `desktop.App` 的导出方法与包级事件（见 [shell.md](shell.md) §4）。生成的客户端给出：

```ts
Desktop.info(): Promise<Info>
Desktop.sessions(cwd: string): Promise<SessionRow[]>
Desktop.newSession(cwd: string, model: string, thinking: string): Promise<string>
Desktop.openSession(path: string, model: string, thinking: string): Promise<string>
Desktop.history(sessionId: string): Promise<Message[]>
Desktop.send(sessionId: string, text: string): Promise<void>
Desktop.stop(sessionId: string): Promise<void>
Desktop.setModel(sessionId: string, model: string): Promise<void>
Desktop.setThinking(sessionId: string, level: string): Promise<void>
Desktop.models(): Promise<string[]>

events.harnessEvent.on((e: Envelope) => void)
```

- 事件 `Envelope` 字段：`kind`、`sessionId`、`payload`；`payload` 形状由 `eventbus` DTO 定义（`MessageDTO`、`MessageUpdateDTO`、`ToolExecutionDTO` 等）。
- 生成物出现差异时以 Go 签名为准；不允许在 TS 里补字段、改返回类型。
- 错误：`CallError` 的 `message` 直接显示；`send` 的取消不产生错误。

## 4. 页面结构

- 单页、无路由；DOM 由 `main.ts` 构建。
- 语义元素：`<aside>` 会话列表、`<main>` 消息区、`<form>` 输入区；按钮有可读标签与焦点样式。
- 主题：跟随系统深浅色（`prefers-color-scheme`）；CSS 变量集中定义；不提供主题编辑器。
- 字体：系统 UI 字体；等宽用于代码块；不下载字体。
- 语言：Phase 3 界面文案中文，代码/标识符英文。

## 5. 渲染与状态

- 消息列表按 `messageId` 建行，但 DOM 只挂载视口附近（上下 spacer + 实测行高，窗口外约 ±800 px）；`message_update` 只改模型，按 `requestAnimationFrame` 合批刷新一次。
- 5,000 条消息：内存保留全部行数据，挂载约 20 行；滚动时重排窗口并保持滚动锚点；用户上滚后暂停自动跟随，出现“回到底部”。
- 工具卡片：`tool_execution_start` 建卡，`update` 追加进度，`end` 写结果与耗时/错误；`toolResult` 消息并入对应卡片，不再单独冒泡，重启后历史卡片与实时一致。
- 系统消息不渲染；用户消息只由 `message_start/end` 回显（不做本地乐观插入）。
- 忙碌态：`send` 进行中禁用发送、显示停止；`agent_end` 恢复。
- 会话切换：先停止当前 turn，再重新拉取 `history`；旧会话事件丢弃。
- 空状态：无会话时显示“新建会话”；无 key 时状态行提示“凭证：未配置”（不显示明文）。

## 6. 安全

- 页面只加载 `mygo://localhost` 自带资源；不访问外网、不加载 CDN、不做 `eval`/动态 `import` 远程代码。
- CSP（随 `index.html` 响应头或 meta）：`default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'none'; frame-src 'none'`。
- 不把 key、会话文件路径以外的敏感数据带进页面；设置页只显示“已配置/未配置”。
- 不设置 `TrustedOrigins: ["*"]`；窗口只打开应用页面。
- 外链：Phase 3 不内嵌浏览器；如出现链接，交系统浏览器（后续实现）。

## 7. 开发与调试

- 快速循环：`npm run build` → `go run ./cmd/locowrk -cwd <workspace>`；可选 `mygo dev`（需要 `mygo.config.ts`，Phase 3 不作为必需）。
- 隐藏窗口冒烟：`go run ./cmd/locowrk -smoke [-smoke-scenario faux|real|tool|cancel|long|restore]`。场景脚本在 `src/smoke.ts`，通过 `?smoke=<场景>` 触发，断言真实 DOM 并以 JSON 输出；退出码 0/1。
- 真实 provider：`-smoke -smoke-scenario real`，key 只从环境变量读取；`-smoke` 默认临时 cwd，不写仓库。
- 长会话：`node scripts/seed-long-session.mjs <home> <cwd> 5000` 造会话文件，再用 `restore` 场景加载并计时。
- 调试：页面 `console` 由 WebView 开发者工具查看（开发构建）；生产构建不启用远程调试。

## 8. 体量与验收

- 前端产物合计 ≤10 MB、单文件 ≤1 MB；无位图/字体资产。
- 构建可复现：Node 版本写入 `web/package.json` 的 `engines`；`package-lock.json` 入库。
- 验收对齐 Phase 3 §5：流式显示、工具卡片、取消、恢复、空闲 CPU、5k 滚动、无密钥泄漏。
- 偏离登记：若 `mygo generate` 或 esbuild 在阶段内不可用，允许先用 `window.mygo` 的等价调用完成界面，但必须在 Phase 3 报告登记，并在 Phase 5 前切换到生成式客户端。
