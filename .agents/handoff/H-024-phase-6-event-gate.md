# H-024：Phase 6 事件闸门与会话切换确认

- 日期：2026-10-30
- 主题：`wantsEventAck` 请求/回执、内核 `BeforeTool` 钩子、`session_before_switch`

## 本轮做了什么

- shim：`initialize` 结果增加 `wantsEventAck: true`；带 id 的 `event` 请求在分发完后回 `event_result`，把多个处理器的返回值合并为 `{block, reason}` 或 `{cancel}`（block 优先，保留首个 reason）。
- 适配器：记录 `wantsEventAck`，新增 `WantsEventAck()` 与 `RequestEvent(ctx, kind, payload)`（预算 = RPC 超时 + 对话框超时，避免权限对话框被提前掐断）。
- 管理器：`GateToolCall`（首个 block 生效；插件出错记通知并放行）与 `ConfirmSessionSwitch`（`cancel` 中止切换）。
- 内核：`SessionOptions/OpenOptions.BeforeTool`（`BeforeToolFunc`），映射 pi `BeforeToolCall`，返回非空 reason 即阻断；bridge 透传；`cmd/locowrk` 接 `extMgr.GateToolCall`。
- 桌面：`desktop.Options.BeforeSessionSwitch`，新建/打开会话前调用；启动器接 `extMgr.ConfirmSessionSwitch`。
- 夹具：`orchestrator-sample` 增加 `tool_call` 阻断样本（`danger` 或含 `rm -rf`），重新打包。
- 测试：`TestToolCallGate`、`TestBeforeToolGateBlocksExecution`；`TestRealPiSamples` 增加危险 `bash` 阻断与 `session_before_switch` 取消/放行断言。

## 关键结果

- 真实 `rpc-demo`：危险命令触发 select 对话框并被阻断（notify「Command blocked by user」）；拒绝确认则会话切换被取消，确认则放行。
- `go test -race ./...`、夹具 `--check`、`plugin`/`steer` 冒烟全绿。
- 修正 steering 测试时序（P-011）：补充指令必须在流式输出中发出。

## 遗留（Phase 6 未收尾）

1. 技能信任/撤销 UI 冒烟。
2. 真实 Node 崩溃/超时/进程树/工具冲突注入；发布缺运行时 fail-closed。
3. `ctx.sessionManager` 无真实会话条目；页面内标签切换没有 `session_before_switch` 触发点。
4. 阻塞工具时 `Terminate` 未使用（当前只阻断单次调用）。

## 已提取的沉淀

- 新增 P-011（steering 时机）；P-010 仍适用于网络。
