# 设计：Phase 3 最小桌面主壳

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0011、ADR-0012、ADR-0002；architecture/01、02、03；[frontend.md](frontend.md)；[MyGo spike](../../research/03-mygo-stack.md#8-phase-3-spike-结果2026-10-28)

## 1. 目标

一个能日常用的最小桌面窗口：看到会话列表、逐字看到回答、调用工具时看到过程、能停止、能重启后接着用。

不做：插件插槽（Phase 4）、Diff（Phase 5）、主题、多窗口、会话搜索。

## 2. 组成与依赖方向

```
cmd/locowrk        flags、装配、退出码；不写界面逻辑
desktop/           MyGo 窗口、绑定服务、事件泵、前端资源 embed
internal/bridge    DTO-only 门面（已落地）
internal/kernel    内核封装（唯一引用 pi/ 的正式代码）
web/               前端源码与构建产物（见 frontend.md）
```

- `cmd/locowrk` 只做三件事：解析参数 → 组装 `kernel.Service`、`bridge.Service`、`desktop.App` → `mygo.App.Run()`。
- `desktop` 可以 import MyGo；`internal/bridge`、`internal/kernel` 不得 import MyGo。
- 前端只通过 MyGo 绑定方法调用 Go，不直接读内核/会话文件。

## 3. 启动与关闭

启动顺序：

1. 解析参数：`-cwd`、`-model`、`-thinking`、`-faux`、`-smoke`。
2. `kernel.New`（`-faux` 时用 `kernel.WithFauxModel`）；`bridge.New`，`OnEvent` 指向 `desktop.App` 的事件泵。
3. `mygo.BindAs("Desktop", app)`；`mygo.SetFrontend(webDist)`；`App.WhenReady` 里建窗口并加载 `/`。
4. 页面加载后调用 `Desktop.info()` 拿 cwd/模型/思考档，再 `Desktop.sessions(cwd)`。

关闭：窗口关闭 → 取消进行中的 turn（`bridge.Cancel`）→ `bridge.Close`（退订）→ `kernel.Close`（落盘）。不弹“确定退出”。

## 4. 绑定契约（Desktop 服务）

所有入参/出参是项目 DTO 或 JSON 原生类型；内核类型不出现。

| 方法 | 入参 | 返回 | 说明 |
|---|---|---|---|
| `Info` | — | `{cwd, model, thinking, version}` | 启动时的默认值；模型来自 `-model` 或环境 |
| `Sessions` | `cwd` | `[]{id, path, timestamp, messages}` | 重启后可见；按上游排序 |
| `NewSession` | `cwd, model, thinking` | `sessionId` | 成功后自动 `Watch` |
| `OpenSession` | `path, model, thinking` | `sessionId` | 失败返回错误文本 |
| `History` | `sessionId` | `MessageDTO[]` | 打开会话时填充消息流 |
| `Send` | `sessionId, text` | — | 阻塞到本轮结束；取消不算错误 |
| `Stop` | `sessionId` | — | 映射 `bridge.Cancel` |
| `SetModel` / `SetThinking` | `sessionId, value` | — | 影响下一轮 |
| `Models` | — | `string[]` | Phase 3 固定候选，不做模型市场 |

事件：包级 `var Event = mygo.NewEvent[Envelope]("harness-event")`，`Envelope{kind, sessionId, payload}`；`payload` 是 `eventbus.Event` 的 JSON 对象。`Send` 期间事件按 `Seq` 到达页面。

错误：方法返回 `error` 时 MyGo 转成前端 `CallError`；界面显示错误文本，不吞掉。`kernel.ErrCanceled` 在 `Send` 内转成正常返回，由 `agent_end`/`turn_end` 收尾。

## 5. 界面（最小可用）

```
┌────────────────────────────────────────────────────┐
│ Locowrk        [模型 ▾] [思考档 ▾]                  │
├──────────────┬─────────────────────────────────────┤
│ 会话          │  消息流                              │
│ + 新建        │   user / assistant / tool 卡片       │
│ 最近…         │                                      │
│              ├─────────────────────────────────────┤
│              │ [输入框................] [发送][停止] │
└──────────────┴─────────────────────────────────────┘
```

- 左侧：`Sessions(cwd)`；点击 `OpenSession(path)`；新建用当前 cwd + 顶部模型/思考档。
- 顶部：模型与思考档；切换对当前会话调用 `SetModel`/`SetThinking`，下一轮生效。
- 消息流：用户消息即时插入；助手文本按 `message_update` 增量追加（按 `messageId` 归并）；`message_end` 用完整文本校正；工具调用显示名称、参数、结果/错误与耗时（`tool_execution_*`）；`message_update` 合并失败时退化为逐条追加。
- 输入：多行；Enter 发送，Shift+Enter 换行；发送后清空；`Send` 返回前禁用发送、显示“停止”。
- 停止：调用 `Stop`；界面显示“已停止”，已产生内容保留，可继续发下一条。
- 自动滚动：默认跟随；用户向上滚动后暂停，出现“回到底部”按钮。
- 设置：只显示数据目录与凭证状态（已配置/未配置），不显示 key 明文；Phase 3 不做编辑。

## 6. 事件到界面的映射

| 事件 | 界面动作 |
|---|---|
| `agent_start` / `agent_end` | 忙碌态开始/结束；结束恢复输入框 |
| `turn_start` / `turn_end` | 轮次分隔；`turn_end` 可携带工具结果 |
| `message_start` | 新建消息占位（user/assistant） |
| `message_update` | 按 `messageId + blockIndex` 追加文本；`toolcall_delta` 暂不渲染 |
| `message_end` | 用 `MessageDTO` 校正文本、stopReason、错误 |
| `tool_execution_start` | 工具卡片“运行中”，显示参数 |
| `tool_execution_update` | 工具卡片追加进度文本 |
| `tool_execution_end` | 完成/失败、耗时、结果摘要 |
| `plugin_diagnostic` | Phase 4 起用；Phase 3 只记录控制台 |

## 7. 状态与恢复

- 会话文件仍是唯一事实源（ADR-0005）；前端不缓存可恢复状态。
- 打开会话：`OpenSession` → `History` → 渲染；进行中会话的事件只影响当前选择。
- 切换会话：先 `Stop` 当前 turn（若在跑），再取消订阅旧会话。
- 重启：`Sessions(cwd)` 列出磁盘会话；`phase-3` 验收要求消息与工具结果与关闭前一致。

## 8. 性能

- 事件：`message_update` 增量按消息归并；页面按动画帧刷新（`requestAnimationFrame`），避免每个 delta 一次 DOM 操作。
- 长会话：虚拟化消息列表；Phase 3 验收用 5,000 条消息，滚动不掉帧、输入不卡。
- 空闲：无轮询；只有事件到达时更新。窗口静置 60 秒 CPU 接近 0。
- 内存：不保留整段重复文本；消息节点按视口挂载。

## 9. 测试与验收

- 自动：`bridge` 契约测试（已落地）；`desktop` 冒烟模式 `-smoke`：隐藏窗口 + faux provider，走“新建 → 发送 → 收到助手文本 → 停止/退出”，断言 DOM 中出现回答，退出码 0。
- 手动：Phase 3 §5 的 1~8 条（真实 key、工具、取消、恢复、空闲、5k、边界、安全）；中文 IME 与三平台在高 DPI/输入法下的表现。
- 边界检查：`go list -deps ./internal/bridge` 不含 MyGo；`desktop` 之外无 MyGo import。

## 10. 风险与未决

- 前端构建链（TypeScript/Vite/mygo generate）见 [frontend.md](frontend.md)；若工具链阻塞，先提交最小内联页面，但必须在 Phase 3 报告登记偏离。
- MyGo v0.x 漂移：用法收敛在 `desktop/`；升级独立提交。
- WebView 焦点/层级/DPI：弹层统一由宿主管理；Phase 3 冒烟清单覆盖。
- 会话列表排序：当前用上游“header 时间”；产品要“最后活动时间”时再改内核/kernel。
