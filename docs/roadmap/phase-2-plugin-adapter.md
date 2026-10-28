# Phase 2：编排插件适配器骨架与 A 轨运行时选型

- 状态：草稿
- 日期：2026-10-28
- 关联：ADR-0003、ADR-0004、ADR-0008；architecture/03、04；research/02

## 1. 目标

把 A 轨的**运行时选型、协议、三分类兼容、进程生命周期、诊断**跑通：一个 mock 编排插件以子进程启动，支持项工作、缓做项返回“未实现”、TUI 项 no-op 且不中断；同时用限时 spike 决定真实扩展的运行时（ADR-0008）。

## 2. 开工前置条件

- Phase 1 完成并打 tag。
- JSON-RPC 协议草案与 manifest schema 定稿（写入 `proto/` 与本文）。
- 进程隔离方案（stdin/stdout 帧格式、超时、心跳）确认。
- 一个真实 pi 扩展样本（脱敏、固定版本）可用于 spike；不把它接进生产路径。

## 3. 做什么

### 3.0 A 轨运行时 spike（ADR-0008）

限时 5 个工作日，并行验证：

- **A. QuickJS + 打包**：bundler 把样本扩展打成单文件，注入 pi shim，执行并回收结果。
- **B. Node 兼容子进程**：独立进程运行样本，宿主经 JSON-RPC 提供 shim API。
- **C. Go 重写**：按样本语义重写，作为兜底。

必须跑通：启动 → 注册工具 → 事件钩子 → `notify` 映射 → `setWidget` no-op + 诊断。记录启动时间、内存、Windows 可用性、shim 覆盖面与维护成本，产出 `docs/design/phase-2/runtime-spike.md` 与 ADR-0009。

### 3.1 协议

- JSON-RPC 2.0 over stdio；逐行 JSON 或长度前缀帧（二选一，写入协议文档）。
- 方法：`initialize`、`register_tool`、`register_command`、`emit_event`、`spawn_subagent`、`workflow_step`、`shutdown_plugin`。
- 错误码：`unsupported`、`invalid_params`、`timeout`、`crashed`、`protocol_error`。
- 协议版本与 manifest 版本校验；不兼容直接拒绝启动。

### 3.2 适配器

- `internal/adapter`：`Load` / `Start` / `Events` / `SendCommand` / `Tools` / `Stop`。
- 三分类：安全能力未声明 = 拒绝；支持项真实实现；缓做项返回 `not_implemented`；TUI 项 no-op + `unsupported` 诊断，永不 throw。
- 工具注册：转成 `agent.AgentTool`，参数用 JSON Schema 校验。
- 事件翻译：编排事件补 `SessionID` / `Seq` 后进 EventBus。

### 3.3 Mock 插件与故障注入

- `fixtures/plugins/mock-orchestrator`：一个最小可执行程序，支持 `single` / `parallel` / `supported-ui` / `deferred` / `tui-noop` / `crash` / `hang` 模式。
- 三分类验证：支持项真实返回；缓做项返回 `not_implemented`；TUI 项返回零值并留 `unsupported` 诊断。
- 故障注入：启动即退出、运行中崩溃、RPC 超时、心跳丢失、输出超量。

## 4. 不做什么

- 不把真实 pi 扩展接入生产路径（spike 可运行样本，Phase 6 才正式接入）。
- 不做插件 UI（Phase 4/7）。
- 不引入 gRPC（除非协议评审明确需要，并写 ADR）。
- 不在运行期执行 `npm install` 或远程加载代码。

## 5. 验收场景

1. **运行时定稿**：spike 完成，ADR-0009 选定路线并写明理由、读数与淘汰项。
2. **启动与握手**：mock 插件启动后 10 秒内完成握手，宿主显示“就绪”。
3. **事件转发**：mock 发出子代理/工作流事件，EventBus 收到对应 `subagent_*` / `workflow_step`，顺序正确。
4. **支持项**：工具注册、命令注册、`notify` 映射可用；调用不抛错。
5. **缓做项**：session 写入等成员返回 `not_implemented` 诊断，扩展继续执行。
6. **TUI no-op**：mock 调用 `ctx.ui.setWidget`，返回零值、扩展继续、诊断只出现一次（`count` 累加）。
7. **安全拒绝分开**：mock 请求未声明的文件写能力，被安全拒绝；该记录不与兼容诊断混在一起。
8. **崩溃隔离**：mock 运行中崩溃，宿主进程不退出；记录退出码与脱敏 stderr；按策略重试或禁用。
9. **超时**：mock 故意不响应，RPC 在 30 秒内返回 `timeout`；宿主可继续操作。
10. **清理**：停止插件后无残留进程；孙进程不持有管道（读取有界）。
11. **安全**：插件拿不到未声明的文件/网络能力；日志中无凭证。

## 6. 交付物

- `proto/` 协议文档与 schema。
- `internal/adapter` 与测试。
- mock 插件与故障注入用例。
- `docs/design/phase-2/runtime-spike.md` 与 ADR-0009（运行时选择）。
- Phase 2 报告（含三分类验证、风险、Phase 6 接入建议）。

## 7. 风险

- 三条运行时路线都超预算：先保 A（QuickJS）可用，C 作为兜底；若只能 C，必须把“兼容 pi 扩展”改成“兼容编排语义”。
- 协议过早复杂化：先覆盖 mock 需要的成员，其余留扩展位。
- 管道/进程清理在 Windows 上有坑：必须写故障测试，不靠日志推断。
- 三分类与真实 pi 语义可能不一致：Phase 2 用上游源码复核，本文与 architecture/04 先改再改代码。
