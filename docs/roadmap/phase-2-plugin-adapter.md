# Phase 2：编排插件适配器骨架

- 状态：草稿
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004；architecture/03、04；research/02

## 1. 目标

在不接真实 pi 扩展的前提下，把 A 轨的**协议、能力过滤、进程生命周期、诊断**跑通：一个 mock 编排插件以子进程启动，事件能转发，`ctx.ui.*` 被拒绝且主流程不中断。

## 2. 开工前置条件

- Phase 1 完成并打 tag。
- JSON-RPC 协议草案与 manifest schema 定稿（写入 `proto/` 与本文）。
- 进程隔离方案（stdin/stdout 帧格式、超时、心跳）确认。

## 3. 做什么

### 3.1 协议

- JSON-RPC 2.0 over stdio；逐行 JSON 或长度前缀帧（二选一，写入协议文档）。
- 方法：`initialize`、`register_tool`、`register_command`、`emit_event`、`spawn_subagent`、`workflow_step`、`shutdown`。
- 错误码：`unsupported`、`invalid_params`、`timeout`、`crashed`、`protocol_error`。
- 协议版本与 manifest 版本校验；不兼容直接拒绝启动。

### 3.2 适配器

- `internal/adapter`：`Load` / `Start` / `Events` / `SendCommand` / `Tools` / `Stop`。
- 能力过滤：未声明 = 拒绝；`ui` / `shortcut` 返回结构化错误与一次性诊断。
- 工具注册：转成 `agent.AgentTool`，参数用 JSON Schema 校验。
- 事件翻译：编排事件补 `SessionID` / `Seq` 后进 EventBus。

### 3.3 Mock 插件与故障注入

- `fixtures/plugins/mock-orchestrator`：一个最小可执行程序，支持 `single` / `parallel` / `ui-call` / `crash` / `hang` 模式。
- 故障注入：启动即退出、运行中崩溃、RPC 超时、心跳丢失、输出超量。

## 4. 不做什么

- 不加载真实 pi TypeScript 扩展（Phase 6）。
- 不做插件 UI（Phase 4/7）。
- 不引入 gRPC（除非协议评审明确需要，并写 ADR）。

## 5. 验收场景

1. **启动与握手**：mock 插件启动后 10 秒内完成握手，宿主显示“就绪”。
2. **事件转发**：mock 发出子代理/工作流事件，EventBus 收到对应 `subagent_*` / `workflow_step`，顺序正确。
3. **UI 拒绝**：mock 调用 `ctx.ui.overlay`，收到 `ErrUIUnsupported`；同一插件的工具注册与后续事件不受影响；诊断只出现一次。
4. **崩溃隔离**：mock 运行中崩溃，宿主进程不退出；记录退出码与脱敏 stderr；按策略重试或禁用。
5. **超时**：mock 故意不响应，RPC 在 30 秒内返回 `timeout`；宿主可继续操作。
6. **清理**：停止插件后无残留进程；孙进程不持有管道（读取有界）。
7. **安全**：插件拿不到未声明的文件/网络能力；日志中无凭证。

## 6. 交付物

- `proto/` 协议文档与 schema。
- `internal/adapter` 与测试。
- mock 插件与故障注入用例。
- Phase 2 报告（含协议决策、风险、Phase 6 接入建议）。

## 7. 风险

- 协议过早复杂化：先覆盖 mock 需要的 7 个方法，其余留扩展位。
- 管道/进程清理在 Windows 上有坑：必须写故障测试，不靠日志推断。
- “能力过滤”与真实 pi 语义可能不一致：Phase 6 用上游源码复核，本文与 architecture/04 先改再改代码。
