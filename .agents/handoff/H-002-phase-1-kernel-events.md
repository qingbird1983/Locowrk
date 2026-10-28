# H-002：Phase 1 内核接入与事件流

- 日期：2026-10-28
- 主题：`sky-valley/pi` 接入、项目事件总线、JSONL 会话恢复、冒烟壳

## 本轮做了什么

1. `go mod init github.com/qingbird1983/locowrk`；`sky-valley/pi` 锁定解析版本 `v1.4.29-0.20261024235732-6f6131bae0dc`（提交 `6f6131bae0dc`），传递依赖 `x/text`、`x/image` 登记进 standards/04 附录 A。
2. `internal/eventbus`：`Kind`(10 生命周期 + 5 编排)、`Event{Seq,Kind,SessionID,At,Payload}`、DTO（message/message_update/tool_execution/turn_end/agent_end）；`Bus` 按会话分序号，每订阅者有界队列 + 投递协程；delta 合并、溢出计数、超时断开。
3. `internal/kernel`：唯一 import 上游的包；`New/CreateSession/OpenSession/Prompt/Steer/Abort/History/SetModel/SetThinking/SessionPath/Close`；事件翻译成项目 DTO；取消映射为 `ErrCanceled`。
4. `cmd/locowrk-smoke`：JSONL 事件到 stdout，`-resume`、`-faux`。
5. 7 条验收全过（faux 驱动，`-race`）；首次规范复查完成；Phase 1 报告、看板、CHANGELOG、docs/README 已更新。

## 遗留与下一步

- Phase 2 文档仍是草稿，需先定稿；A 轨运行时 spike 5 天，结论写 ADR-0009。
- `Prompt` 目前阻塞；Phase 3 需要异步门面，不要在 Phase 2 里改内核语义。
- 真实 provider 冒烟待有 key 的环境；不阻塞 Phase 2。
- 事件 DTO 暂不传工具结果图片（只有文本），Phase 3 展示图片时加字段。

## 已提取的沉淀

- `pitfalls/P-002-pi-writes-to-home.md`：上游会话目录固定在 `~/.pi`，测试必须换主目录。
- 复用 `P-001`：Phase 1 未新增 ADR，状态事实只写看板/CONTEXT/CHANGELOG。
