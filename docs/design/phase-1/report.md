# Phase 1 阶段进度报告（写给非技术读者）

- 日期：2026-10-28
- 状态：**完成**（验收全部通过，tag `phase-1.end`）
- 本阶段目标（一句话）：把 `sky-valley/pi` 这个“大脑”接进来，让一次完整对话（含工具调用）能跑通、事件能实时看到、会话能存下来并恢复；本阶段不做界面。

## 完成了什么（3~5 条，无行话）

1. **接上内核**：模块 `github.com/qingbird1983/locowrk` 建立，依赖锁定在 `sky-valley/pi` 的提交 `6f6131bae0dc`（解析版本 `v1.4.29-0.20261024235732-6f6131bae0dc`）。
2. **统一事件流**：10 类生命周期事件（开始/结束、轮次、消息、流式增量、工具开始/进度/结束）加 5 类编排事件，统一定义为项目自己的 `Event`，带会话号与单调递增序号；上游结构不直接外泄。
3. **慢消费者不会卡住大脑**：每个订阅者有自己的有界队列与投递协程；文本增量会合并，队列溢出时丢弃增量并计数，控制类事件溢出则断开该订阅者并记录原因。
4. **会话能存能恢复**：新建/续接 JSONL 会话（沿用上游格式），历史、模型切换、思考档都写入文件；`OpenSession` 可从文件恢复上下文继续对话。
5. **可手动冒烟**：`cmd/locowrk-smoke` 能把事件按 JSONL 打到标准输出，支持 `--resume` 与 `--faux`（无 key 也能跑）。

## 验收场景执行结果（逐条：通过/未通过 + 一句说明）

| # | 场景 | 结果 | 说明 |
|---|---|---|---|
| 1 | 完整工具调用 | 通过 | faux 模型先 `read` 再回答；事件序列含 `agent_start` → `turn_start` → `message_*` → `tool_execution_start/end` → `agent_end`，工具结果文本回传正确（`TestToolCallRunEmitsLifecycleAndPersists`） |
| 2 | 会话持久化 | 通过 | 运行后 JSONL 存在；关闭服务后 `OpenSession` 恢复历史，再对话文件继续增长（`TestOpenSessionResumesAndAppends`） |
| 3 | 取消 | 通过 | 流式过程中取消，`Prompt` 返回 `ErrCanceled`，实测 < 2 秒返回；无 goroutine 泄漏，`-race` 全绿（`TestPromptCancellationReturnsCanceled`） |
| 4 | 多会话隔离 | 通过 | 两个会话并发运行，事件 `SessionID` 不串，`Seq` 各自从 1 单调递增（`TestSessionsAreIsolated`） |
| 5 | 背压 | 通过 | 订阅者不读时发布不阻塞；增量合并、溢出计数、超时断开均按 architecture/02 §2.3 执行（`TestSlowSubscriberDoesNotBlockPublisher`、`TestControlOverflowDisconnects`） |
| 6 | 边界 | 通过 | `go list -deps ./internal/...` 不含 `egoist/mygo`；`gofmt -l` 为空；`go vet`、`go test -race ./...` 全绿 |
| 7 | 安全 | 通过 | 事件 payload 中不含 API key（`TestEventsDoNotLeakAPIKeys`）；测试用临时 `HOME`/`USERPROFILE`，不写用户真实目录 |

## 交付物

- `internal/eventbus/`：`Event`/`Kind`/DTO 定义 + 有界队列、合并、溢出与断开策略（含 6 个单测）。
- `internal/kernel/`：`Service`（建会话、续接、Prompt、Steer、Abort、History、换模型/思考档、Close）+ 事件翻译 + 5 个集成测试。
- `cmd/locowrk-smoke/`：临时冒烟壳，Phase 3 删除。
- 依赖：`github.com/sky-valley/pi` 固定提交；其传递依赖 `golang.org/x/text`、`golang.org/x/image` 随附（BSD-3，见 standards/04 附录 A）。

## 体量核对（standards/08）

- docs/：40 篇 + 本报告 = 41 篇（软上限 120）；超限篇目：无；单篇最大为架构方案（<300 行）。
- 代码：新增 7 个 Go 文件共约 1718 行（含测试 515 行）；无 >500 行文件；新增依赖 1 个直接依赖 + 2 个传递依赖。
- `.agents/`：4 个文件（上限 40）；CONTEXT 未超 150 行。
- 前端资产：0 MB。
- 豁免登记：无。

## 规范复查（standards/09 首次例行复查）

按 standards/09 §3 四问逐篇过一遍，结论如下（无发现即如实记录）：

- **有漏洞吗**：发现 1 处。standards/04 §5 依赖准入表只列直接依赖，未说明上游带来的传递依赖如何登记；已在附录 A 补两行并注明“随 `sky-valley/pi` 传递引入”。其余篇目未发现本阶段坏结果。
- **过严吗**：`phase-1` 要求“取消 2 秒内返回”对本地 faux/真实 provider 都可达，未造成无用功；无豁免。standards/02 的“禁止直接 push main”在只有单一所有者时略重，但保留了首次初始化例外，暂不改。
- **过松吗**：测试隔离靠 `t.Setenv` 后置环境变量，属“靠自觉”；已在 kernel 测试里集中成 `testHome(t)` 辅助函数，避免每处手写。是否升级为规范硬规则留待 Phase 2 观察。
- **冗余或矛盾吗**：architecture/02 §2.2 的事件表与 `eventbus.Kind` 常量已对齐；未发现矛盾。standards/04 §5 的“每包直接依赖 ≤15”与当前 1 个直接依赖无冲突。

## 已知问题与风险

- **真实 provider 未跑**：验收使用 faux 与录制式流；接入真实模型前需要在有 key 的环境跑一次（不阻塞 Phase 2）。
- **Prompt 目前是阻塞调用**：桌面端需要“边跑边可控”，Phase 3 会在 `kernel.Service` 外加异步门面（`Start`/`Wait`），不改动本阶段语义。
- **事件 DTO 仍是 v0**：`message_update` 的块索引/累计快照已稳定，但工具结果的图片内容暂只传文本；Phase 3 展示图片时再扩展，属向后兼容新增字段。
- **会话目录固定**：`~/.pi/agent/sessions/` 由上游决定，无环境变量覆盖；测试靠临时主目录，用户迁移需求进 backlog。
- **单机无 CI**：本阶段门禁全在本地跑；升级内核时仍须手工全量 `go test -race ./...`。

## 对下阶段的影响

- Phase 2 可以直接在 `kernel.Service` 之上写编排适配器，不需要碰上游 `agent` 包；事件种类已冻结，适配器只新增编排级事件。
- 插件诊断事件 `plugin_diagnostic` 已在 `eventbus` 预留，Phase 2 只需定义 payload。
- 取消与背压语义已验证，Phase 3 的 UI 可以直接依赖“Agent 循环不会因界面慢而停摆”。

## 文档更新清单

- 新建：本报告、`handoff/H-002-phase-1-kernel-events.md`。
- 更新：`docs/README.md`（登记本报告、Phase 1 状态）、`docs/research/01`（实际解析版本）、`docs/standards/04`（传递依赖）、`docs/roadmap/00-roadmap.md`（看板）、`CHANGELOG.md`。
