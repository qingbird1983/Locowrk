# 架构 02：内核封装与事件模型

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0001、ADR-0005；architecture/03、05；research/01

## 1. 依赖边界

`internal/kernel` 是**唯一** import `github.com/sky-valley/pi` 的包。它向上暴露项目自己的类型，不让上游类型泄漏到 UI 与插件协议。

```go
package kernel

type Service struct {
    agents   map[string]*coding.Session // 上游类型只在包内
    bus      *eventbus.Bus
    dataDir  string
}

func New(opts Options) (*Service, error)
func (s *Service) CreateSession(ctx context.Context, opts SessionOptions) (SessionID, error)
func (s *Service) Prompt(ctx context.Context, id SessionID, text string, images []Image) error
func (s *Service) Steer(ctx context.Context, id SessionID, text string) error
func (s *Service) Abort(ctx context.Context, id SessionID) error
func (s *Service) History(ctx context.Context, id SessionID) ([]Message, error)
func (s *Service) Close(ctx context.Context) error
```

- 上游包映射：`ai`（模型/流/工具 schema）、`agent`（循环/事件/队列）、`coding`（会话/工具/系统提示/技能/压缩）、`telemetry`（可选，默认关闭）。
- `chord`、`env` 当前不接入：前者是协同编辑增量，后者是 SSH 远程执行；与本项目 M0~M7 无关。需要时单独立项。

## 2. 事件模型

### 2.1 上游已经给了什么

`agent.Agent` 通过 `Subscribe(Listener)` 发出 10 类生命周期事件：

| 上游常量 | 值 | 用途 |
|---|---|---|
| `EvAgentStart` | `agent_start` | 一次运行开始 |
| `EvAgentEnd` | `agent_end` | 一次运行结束，带完整消息列表 |
| `EvTurnStart` | `turn_start` | 一轮（一次模型请求）开始 |
| `EvTurnEnd` | `turn_end` | 一轮结束，带工具结果 |
| `EvMessageStart` | `message_start` | 消息开始 |
| `EvMessageUpdate` | `message_update` | 流式增量，`AssistantMessageEvent` 指向 `ai` 的流事件 |
| `EvMessageEnd` | `message_end` | 消息结束 |
| `EvToolExecutionStart` | `tool_execution_start` | 工具开始，带 id/name/args |
| `EvToolExecutionUpdate` | `tool_execution_update` | 工具进度 |
| `EvToolExecutionEnd` | `tool_execution_end` | 工具结束，带结果/错误/耗时 |

流式增量内部使用 `ai.EventType`：`start`、`text_start/delta/end`、`thinking_start/delta/end`、`toolcall_start/delta/end`、`done`、`error`。

**结论：不要重新发明生命周期事件。** 项目事件只是在上面加会话标识、序号与时间戳，并补编排级事件。

### 2.2 项目事件

```go
package eventbus

type Kind string

const (
    // 与 agent.Ev* 一一对应
    KindAgentStart         Kind = "agent_start"
    KindAgentEnd           Kind = "agent_end"
    KindTurnStart          Kind = "turn_start"
    KindTurnEnd            Kind = "turn_end"
    KindMessageStart       Kind = "message_start"
    KindMessageUpdate      Kind = "message_update"
    KindMessageEnd         Kind = "message_end"
    KindToolExecutionStart Kind = "tool_execution_start"
    KindToolExecutionUpdate Kind = "tool_execution_update"
    KindToolExecutionEnd   Kind = "tool_execution_end"

    // 编排级扩展（来自适配层，不属于上游 Agent 循环）
    KindSubagentSpawned   Kind = "subagent_spawned"
    KindSubagentSettled   Kind = "subagent_settled"
    KindWorkflowStep      Kind = "workflow_step"
    KindSteeringApplied   Kind = "steering_applied"
    KindPluginDiagnostic  Kind = "plugin_diagnostic"
)

type Event struct {
    Seq       uint64          // 每会话单调递增
    Kind      Kind
    SessionID string
    At        time.Time
    Payload   json.RawMessage // 稳定 DTO，不直接序列化上游结构
}
```

`Payload` 使用项目 DTO，避免上游字段改名直接破坏 UI 与插件协议。`message_update` 的 DTO 至少包含：消息 id、块索引、块类型（text/thinking/toolcall）、增量文本、累计快照（可选）。

### 2.3 投递与背压

| 事件类 | 语义 | 策略 |
|---|---|---|
| 控制事件（start/end/工具开始结束/诊断） | 不能丢 | 每订阅者有界队列；满时阻塞**该订阅者**，不阻塞 Agent 循环；超时断开订阅者并记录 |
| 增量事件（`message_update` 的文本/思考 delta） | 可合并 | 同一消息同一块在 16~33ms 窗口内合并；队列满时丢弃旧 delta 并计数 |
| 编排事件 | 不能丢 | 与控制事件同级 |

原则：**Agent 循环永不因 UI 慢而停摆**。UI 断线/重连后靠 `History` + `Seq` 补齐，不靠内存回放。

## 3. 会话接入

- 创建：`coding.NewSession(coding.SessionOptions)`；模型、工具白名单、思考档、压缩设置、审阅钩子一次传入。
- 运行：`Session.Run` / `RunMessages` / `Continue`；取消用 `context.Context`。
- 恢复：`coding.StartSession` 写入首条用户消息时落盘；`coding.LatestSession` / `LoadSessionTree` / `Session.LoadBranch` 恢复。细节见 architecture/05。
- 守卫：桌面端**只通过 kernel.Service 读写会话**，不直接碰磁盘。

## 4. 工具与审阅

- 内置工具：`read`、`write`、`edit`、`bash`、`powershell`、`ls`、`find`、`grep`（`coding.ToolNames`）。
- 工具选择：`SessionOptions.ToolNames` 支持 `*` 通配与 `+name`/`-name` 修饰；`ExcludeTools` 显式排除；`NoToolsAll` 全关。
- 审阅接缝：`SessionOptions.BeforeToolCall` 返回拒绝时，上游不会执行工具；桌面端把它映射为用户确认流程。拒绝必须**零副作用**（不写文件、不 spawn）。
- 工具结果进度：`AgentToolResult` 的 `Update` 回调映射为 `tool_execution_update`。

## 5. 模型、凭证与遥测

- 模型：`ai.BuiltinModels` 从环境或注入的 `ai.CredentialStore` 取凭证；桌面端用 MyGo 的系统钥匙串能力实现 `CredentialStore`。
- 运行期换模型：`Session.SetModel`；思考档：`SetThinkingLevel`。换模型不重建会话。
- 遥测：`sky-valley/pi` 有 telemetry 包，但本项目**默认关闭**；开启须先改 standards/06（零遥测默认）。

## 6. 版本钉与升级

- 当前钉：`github.com/sky-valley/pi` 本地快照 `6f6131ba`（2026-10-25，分支 `main`，描述 `env/v1.4.28-14-g6f6131b`）。
- 升级流程：单独分支 → 跑全量 `go test -race ./...` → 核对 `agent` 事件常量、`coding.SessionOptions`、`ai` 流事件、会话迁移 → 更新 research/01 与 CHANGELOG → 合并。
- 禁止在功能分支里顺手升级内核；禁止用 `replace` 指向本地路径入库。

## 7. 错误处理

- 内核层：错误用 `fmt.Errorf("...: %w", err)` 保留链；对用户可见的错误在 bridge 层翻译为 `{code, message, detail}`，原始错误只进日志（脱敏）。
- 取消：`context.Canceled` 不当作故障；UI 显示“已停止”，会话保留已产生内容。
- panic：上游工具执行已有 recover；本项目在 bridge 与插件边界再加一层 recover，禁止 panic 穿过 IPC。
