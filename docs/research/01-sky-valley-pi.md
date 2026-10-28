# 调研 01：`sky-valley/pi` 内核事实清单

- 状态：定稿（2026-10-28 改为仓库内 fork，见 ADR-0012）
- 日期：2026-10-28
- 关联：ADR-0012、ADR-0005；architecture/02、05；`pi/FORK.md`

> 本文是内核的事实底稿。数据来自基线源码快照与上游自带文档；跟进上游或本地改动内核时必须同步更新本文。

## 1. 身份与版本

| 项 | 值 |
|---|---|
| 模块 | 本仓库 `github.com/qingbird1983/locowrk/pi`（fork 自 `github.com/sky-valley/pi`） |
| 定位 | pi 的纯 Go 移植：`pi-ai` + `pi-agent-core` + `pi-coding-agent` 的 SDK 半边 |
| 许可证 | MIT（含上游 Mario Zechner 版权行，见 `pi/LICENSE`） |
| `go.mod` | 上游 `go 1.26`；本仓库模块 `go 1.27.3` |
| 上游基线 | 提交 `6f6131ba`，2026-10-25，分支 `main` |
| 描述 | `env/v1.4.28-14-g6f6131b`（根模块曾发 `v1.4.28`；`env` 子模块独立打 tag） |
| 导入范围 | `agent`、`ai`（含 `providers`、`models_catalog.json`）、`coding`、`internal/jstext`、`internal/nodepath`、`telemetry` 的非测试源码 |
| 本地维护 | 由本仓库维护；差异登记在 `pi/FORK.md`，行为改动先写 ADR/Phase 文档 |
| 依赖 | `golang.org/x/text`、`golang.org/x/image`（BSD-3，直接 require） |
| 上游对应 | pi TS 源码 pin `38ff99177`；npm `@earendil-works/pi-ai` 1.4.0 |
| CI | **无**（其 UPSTREAM.md 明说仓库不跑 CI，只有本地 `go test`） |

## 2. 包与职责

| 包 | 对应上游 | 提供什么 |
|---|---|---|
| `ai` | `@earendil-works/pi-ai` | 消息/内容/工具/模型类型、基于 channel 的事件流、JSON Schema 校验、模型目录与计费、Provider 注册表、凭证解析、重试、遥测接口 |
| `ai/providers` | `pi-ai/providers` | Anthropic Messages、OpenAI Chat Completions、OpenAI Responses、Google Gemini、faux（测试替身），以及 Azure/OpenRouter/Copilot/Cloudflare/pi-messages 等兼容分支 |
| `agent` | `pi-agent-core` | 低层 `AgentLoop`、有状态 `Agent`、工具调用、钩子、steering/follow-up 队列、串行/并行工具执行 |
| `coding` | `pi-coding-agent` | 8 个内置工具、系统提示词装配、模型解析、会话运行器、JSONL 会话持久化、压缩、技能发现 |
| `cmd/pi` | pi CLI | 手写 SDK CLI：print 模式、交互 REPL、`models`、`sessions`；**不是**上游 modes 的移植 |
| `chord` / `chord/delta` | `@earendil-works/chord` | 协同编辑增量与 tracker；当前与本项目无关 |
| `env` | `@earendil-works/pi-env` | SSH 远程执行环境（独立子模块与 tag）；当前与本项目无关 |
| `telemetry` | `packages/telemetry` | 运行时契约、schema 数据；本项目默认关闭 |
| `internal/jstext`、`internal/nodepath`、`internal/policy` | — | JS 语义对齐、Node 路径语义、依赖白名单门禁 |
| `difftest/` | — | 与真实 pi 的请求体差分测试（独立模块，不随依赖下载） |

## 3. 规模读数（本机快照，排除 `difftest/`）

| 范围 | 非测试文件 | 非测试行 | 测试文件 | 测试行 |
|---|---:|---:|---:|---:|
| 全仓 | 201 | 66,013 | 263 | 78,081 |
| `ai` | 201* | 69,563* | — | — |
| `agent` | 11 | 6,247 | — | — |
| `coding` | 100 | 28,102 | — | — |
| `chord` | 33 | 13,004 | — | — |
| `env` | 78 | 19,150 | — | — |

\* `ai` 的计数包含 `ai/providers` 子目录；与全仓口径不可直接相加，仅用于感知模块体量。

## 4. 能力清单

- **模型**：内置目录约 1,599 个 chat 模型、42 个 provider（README 自述）；4 种线协议覆盖绝大多数模型。
- **工具**：`read`、`write`、`edit`、`bash`、`powershell`、`ls`、`find`、`grep`；工具选择支持 `*` 通配与 `+name`/`-name` 修饰。
- **Agent 循环**：turn 生命周期、工具批处理、串行/并行顺序、`terminate` 早停、steering/follow-up、`before/afterToolCall`。
- **会话**：追加式 JSONL、pi 兼容目录布局、v1→v3 迁移、分支投影、压缩、`context_edit`。
- **系统提示词**：项目上下文文件（`AGENTS.md`/`CLAUDE.md`，`AGENTS.override.md` 覆盖）、技能注入、分区更新。
- **技能**：用户级与项目级 `SKILL.md` 发现、frontmatter 校验、诊断。
- **SDK**：`coding.NewSession` 返回结构化 `RunResult`、`Subscribe` 流、自定义工具、provider 控制面、压缩设置。

## 5. 明确缺口（对本项目最重要）

| 缺口 | 影响 | 应对 |
|---|---|---|
| **没有 extensions runtime**（上游 `core/extensions/**` 被判为宿主面） | A 轨兼容必须本项目自建 | ADR-0003；Phase 2 适配器 + Phase 6 真实插件 |
| **没有 prompt templates**（上游按宿主面排除） | 不能宣称支持 pi 模板 | 需要时自研，先 ADR |
| **没有 TUI / modes / 键位 / 主题** | 与桌面目标一致，不是损失 | 无 |
| 未移植 Bedrock、Codex（WebSocket）、OAuth 获取、部分云厂商适配 | 少数 provider 不可用 | 按需立项；不要假装全支持 |
| **默认不信任项目**：`<cwd>/.pi/skills` 需 `TrustProject` | 否则项目技能静默不加载 | 桌面端做显式授权 UI（接缝 #11） |
| **无 CI** | 上游自身回归无自动防线 | 本仓库门禁；升级跑全量 |
| 会破坏公共 API（其 mirror 策略允许） | 跟进上游时可能需手工移植 | 内核已在仓库内 `pi/`；`internal/kernel` 仍是唯一消费者（接缝 #12） |

## 6. 接入建议

1. 只 import `ai`、`agent`、`coding`；`ai/providers` 通过 `coding` 的导入副作用注册，或按需 blank import。
2. 会话目录由 `coding.DefaultSessionDir` 决定（`~/.pi/agent/sessions/--<cwd>--/`）；`AgentDir` 不读环境变量。测试通过临时主目录（`HOME`/`USERPROFILE`）隔离，不写用户真实目录。
3. 事件直接消费 `agent.AgentEvent`；不要解析 `cmd/pi` 的文本输出。
4. 模型凭证用 `ai.CredentialStore` 注入；不要改 `models.json` 的明文。
5. 内核已在仓库内：本地改动先写 ADR/Phase 文档并登记 `pi/FORK.md`；跟进上游用基线 `6f6131ba` 与新提交做 diff，手工移植后跑 `go test -race ./...`，再核对 `agent.Ev*`、`coding.SessionOptions`、`ai.EventType`、会话迁移。

## 7. 待核实

- 上游 `env` 子模块是否会被引入（当前不需要；若未来做远程执行，单独立项）。
- 模型目录已随 `ai/models_catalog.json` 导入；上游发布更新时手工替换并跑模型相关测试。
- `telemetry` 的默认行为（README 提到安装遥测与 `PI_TELEMETRY=0`）；本项目明确关闭，需在 Phase 1 验证不会意外上报。
