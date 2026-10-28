# Phase 1：内核接入与事件流

- 状态：评审中
- 日期：2026-10-28
- 关联：ADR-0001、ADR-0005；architecture/02、03、05；research/01

## 1. 目标

把 `sky-valley/pi` 接进来，跑通“一次完整工具调用 → 事件序列 → JSONL 会话落盘/恢复”。本阶段**不引入 MyGo**。

## 2. 开工前置条件

- Phase 0 经所有者确认并打 tag。
- 本文状态转「定稿」。
- Go 1.27+ 可用；能拉取 `github.com/sky-valley/pi`（公开仓库）。
- 测试用 provider key 只从环境变量读取；没有 key 时用 faux provider / 录制 fixture。

## 3. 做什么

### 3.1 模块与目录

- `go mod init`（模块路径待所有者确认；未定前用本地模块名，发布前一次改完）。
- 目录：`cmd/locowrk-smoke/`（临时冒烟壳，Phase 3 移除）、`internal/eventbus/`、`internal/kernel/`。
- 固定依赖：`github.com/sky-valley/pi@6f6131ba`（或当时最新的已复核提交；写入 research/01）。

### 3.2 EventBus

- `Event` / `Kind` / `Seq` / `SessionID` 定义（architecture/02 §2.2）。
- `Bus`：订阅、退订、按会话过滤、有界队列、慢订阅者隔离、delta 合并与丢弃计数。
- 事件契约测试：10 类生命周期事件与 `agent.Ev*` 一一映射。

### 3.3 Kernel 封装

- `kernel.Service`：创建/恢复会话、发送消息、取消、读取历史、关闭。
- 订阅 `agent.AgentEvent` 并翻译为项目事件；`message_update` 转 DTO。
- 工具白名单、`BeforeToolCall` 透传、模型与思考档设置。
- 错误翻译：用户可见错误与内部错误分离；取消不算错误。

### 3.4 冒烟验证

- `cmd/locowrk-smoke`：读一个提示词，把事件打到 stdout；支持 `--resume`。
- 用临时主目录（`t.Setenv` 设置 `HOME`/`USERPROFILE`）隔离测试数据；不写用户真实目录。
- 录制事件序列 fixture，作为后续回归基线。

## 4. 不做什么

- 不写 UI、不引入 MyGo、不做插件适配器。
- 不自研模型协议、工具、会话格式。
- 不做 MCP、prompt templates、主题。

## 5. 验收场景

1. **完整工具调用**：让模型（或 faux）先读文件再回答，事件序列包含 `agent_start` → `turn_start` → `message_*`（含 toolcall 增量）→ `tool_execution_start/end` → 最终回答 → `turn_end` → `agent_end`。
2. **会话持久化**：一次运行后 JSONL 文件存在；用 `coding.LoadSessionTree` + `LoadBranch` 恢复，消息、工具结果、模型/思考档与运行前一致。
3. **取消**：流式请求进行中取消，2 秒内返回 `context.Canceled`；已产生的文本仍在会话中；无 goroutine 泄漏。
4. **多会话隔离**：两个会话并发运行，事件 `SessionID` 不串、`Seq` 各自单调。
5. **背压**：人为让一个订阅者阻塞，Agent 循环仍在预算内完成；该订阅者被断开并记录，其他订阅者不受影响。
6. **边界**：`go list -deps ./internal/...` 不含 `egoist/mygo`；`gofmt`、`go vet`、`go test -race ./...` 全绿。
7. **安全**：日志与事件里不出现 API key；测试用临时目录清理干净。

## 6. 交付物

- 可运行的 `locowrk-smoke` 与一段真实/回放事件记录（脱敏）。
- 事件 fixture 与契约测试。
- `docs/design/phase-1/report.md`（含体量核对与首次规范复查）。
- 更新 research/01 的实际依赖版本；必要时新增 ADR。

## 7. 风险

- 上游 API 与文档不一致：以源码与编译为准，发现差异更新 research/01。
- 上游默认安装遥测：验证 `PI_TELEMETRY=0` 或显式关闭，确保零上报。
- 模型 key 缺失导致验收卡住：所有路径都要有 faux/fixture 回放。
- 事件量大：先做合并与有界队列，Phase 3 再用真实 UI 压测。
