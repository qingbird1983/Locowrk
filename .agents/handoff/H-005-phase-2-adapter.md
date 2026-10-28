# H-005：Phase 2 适配器与 mock 落地

- 日期：2026-10-28
- 主题：`internal/adapter`、mock 插件、故障注入、内核外部工具桥

## 本轮做了什么

1. 协议补四个方法（`invoke_tool`、`ui_notify`、`ui_set_widget`、`fs_write`），同步 `proto/plugin-protocol.md`、README 与架构 04；版本不变。
2. `internal/adapter`：manifest 校验、JSONL peer（1MiB 帧、id 归一化、stderr 尾部脱敏）、握手、注册窗口、心跳、RPC 超时、崩溃监控与有限重启、Stop 杀进程树；三分类诊断去重；能力闸 + 独立审计；事件转发到 EventBus（Seq 由 bus 分配）。
3. `fixtures/plugins/mock-orchestrator`：`supported` / `crash` / `hang` 模式，覆盖工具、命令、notify、widget no-op、缓做调用、越权写与事件序列。
4. `internal/kernel`：`ToolSpec` → `agent.AgentTool`，`SessionOptions/OpenOptions.ExtraTools`，faux 回合端到端验证。
5. 测试 12+ 条，`-race` 全绿；顺手把 eventbus 的 flaky 合并断言改成确定性单测。
6. 同步 CHANGELOG 0.2.0、看板、Phase 2 状态、CONTEXT。

## 遗留与下一步

- 孙进程占管道、Stop 的进程树清理测试（acceptance #10）。
- Phase 2 验收逐条复核 + 报告 + `phase-2.end`。
- 迟到注册策略在 Phase 6 真实扩展上复核。
- Node SEA / 随包运行时；真实 provider 冒烟。

## 已提取的沉淀

- `pitfalls/P-004-jsonrpc-id-normalization.md`
