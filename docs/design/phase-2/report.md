# Phase 2 阶段进度报告（写给非技术读者）

- 日期：2026-10-28
- 状态：**完成**（11 条验收场景全部通过，tag `phase-2.end`）
- 本阶段目标（一句话）：让“编排插件”能作为独立进程被启动、注册工具与命令、响应事件、被限时与限权，并在崩溃/超时/越权时保证主程序不受影响；同时用实测选定插件运行时。

## 完成了什么（3~5 条，无行话）

1. **运行时选定**：三条路线实测后选“独立 Node 进程 + 显式 API 子集”（ADR-0009）；证据见 [`runtime-spike.md`](runtime-spike.md)。
2. **插件适配器**：启动、握手（10 秒上限）、注册工具/命令、心跳、单次调用超时、崩溃有限重试、停止杀进程树；全部按 `proto/plugin-protocol.md`。
3. **兼容三分类落地**：支持项真执行；缓做项返回“未实现”诊断；终端界面成员返回空值 + 一次性诊断，重复只累加计数；任何降级都不抛错打断插件。
4. **安全与兼容分账**：未声明或越界的文件写返回 `permission_denied` 并进独立审计；插件日志里的密钥脱敏。
5. **内核能收外部工具**：插件声明的 JSON Schema 直接交给内核校验，模型可真实调用插件工具，结果进会话与事件流；测试用 faux 模型端到端验证。

## 验收场景执行结果（逐条：通过/未通过 + 一句说明）

| # | 场景 | 结果 | 说明 |
|---|---|---|---|
| 1 | 运行时定稿 | 通过 | ADR-0009 选定 Node 子进程；三条路线读数与淘汰项见 `runtime-spike.md` |
| 2 | 启动与握手 | 通过 | mock 启动到 `ready` 实测 **477 ms**（上限 10 s）；工具与命令注册齐全（`TestStartRegistersToolsAndCommands`） |
| 3 | 事件转发 | 通过 | `subagent_spawned` → `workflow_step` ×2 → `subagent_settled` 顺序正确，`SessionID` 正确，`Seq` 从 1 递增（`TestEventForwardingOrder`） |
| 4 | 支持项 | 通过 | 工具 `mock_echo` 返回正确；命令可用；`ui_notify` 映射为宿主通知（`TestSupportedToolAndTUIDiagnostics`） |
| 5 | 缓做项 | 通过 | `spawn_subagent` 返回 `not_implemented`（错误码 4401）+ 诊断，插件继续执行（`TestDeferredHostCall`） |
| 6 | TUI no-op | 通过 | `ui_set_widget` 连续两次返回空值，插件不中断；只留一条 `unsupported` 诊断，`count=2`（`TestSupportedToolAndTUIDiagnostics`） |
| 7 | 安全拒绝分开 | 通过 | 未声明/越界写返回 `permission_denied`（4402），进独立审计；兼容诊断列表为空；目标文件未创建（`TestPermissionDeniedIsSeparateFromDiagnostics`） |
| 8 | 崩溃隔离 | 通过 | 插件以退出码 3 崩溃，宿主进程不受影响；诊断记录退出码；`MaxRestarts=1` 时自动重试一次后禁用（`TestCrashIsolation`、`TestRestartAfterCrash`） |
| 9 | 超时 | 通过 | 插件故意不响应，调用在配置时限内返回 `timeout`，宿主继续可操作（`TestRPCTimeout`） |
| 10 | 清理 | 通过 | 停止后无残留；孙进程占住管道也会被进程树清理杀死；超过 1 MiB 的单帧判 `protocol_error` 并终止插件（`TestStopIsIdempotentAndRestartable`、`TestStopKillsProcessTree`、`TestOversizedFrameIsProtocolError`） |
| 11 | 安全 | 通过 | 未声明写被拒绝且零副作用；协议没有“宿主代管网络”方法，插件原生网络属 ADR-0009 的受信任边界；stderr 中 key/token 被脱敏（`TestRedact`） |

补充：启动不答（握手超时）、manifest 非法（名称/版本/协议/入口逃逸/空范围/全树写/重复能力/未知字段）、路径 glob 与硬拒绝路径均有测试。

## 真实 provider 冒烟（非阻塞项，已完成）

- 时间：2026-10-28；模型：`deepseek/deepseek-flash`（官方文档对应 DeepSeek-V4.1-Flash-1017，1M 上下文、384K 输出、支持工具调用）。
- 接入方式：pi 内置目录已含 `deepseek`，base URL `https://api.deepseek.com`，与官方文档一致；key 只从环境变量读取。
- 场景：临时工作目录放 `note.txt`，提示模型读取并回答冒号后的标记，工具白名单只开 `read`，`-think off`。
- 结果：退出码 0；56 个项目事件；`read` 工具开始/结束成对出现；最终回答 `alpha-42` 正确。
- 隔离：`HOME`/`USERPROFILE` 指向临时目录，未写用户真实 `~/.pi`；日志与事件中无 key。

## 交付物

- `internal/adapter/`：manifest 校验、JSONL peer、握手/心跳/超时/崩溃重启、三分类诊断、能力闸与审计、事件转发（9 个实现文件 + 3 个测试文件）。
- `internal/kernel/tool.go`：`ToolSpec` → 上游 `agent.AgentTool`，`SessionOptions/OpenOptions.ExtraTools`。
- `fixtures/plugins/mock-orchestrator/`：`supported` / `crash` / `hang` / `silent` / `flood` / `grandchild` 模式，以及越权写命令。
- `proto/plugin-protocol.md`：新增 `invoke_tool`、`ui_notify`、`ui_set_widget`、`fs_write`（新增方法，协议版本不变）。
- `docs/design/phase-2/runtime-spike.md` + ADR-0009（运行时）。

## 体量核对（standards/08）

- docs：44 篇（软上限 120）；单篇最大 <300 行；ADR-0009 <100 行。
- 代码：`internal/adapter` 约 2.0k 行（含测试约 0.6k），`internal/kernel` 新增约 0.2k；无 >500 行文件。
- `.agents/`：12 个文件（上限 40）；CONTEXT 未超 150 行。
- 依赖：新增直接依赖 0；测试构建 mock 只用标准库。
- 豁免登记：无。

## 规范复查（简要）

- **有漏洞吗**：协议原先缺“宿主调用插件工具”与 UI/能力请求方法，已补齐并同步文档；属新增方法，不升协议版本。
- **过严吗**：`ReadyGrace` 500 ms 的注册窗口对同步注册的扩展足够；异步注册会在 Phase 6 真实扩展上复核。
- **过松吗**：`Stop(ctx)` 目前忽略调用方 ctx，统一用 `ShutdownTimeout`；已记入 CONTEXT，Phase 3 门面统一时处理。
- **冗余或矛盾吗**：standards/02 要求 feature 分支，但本仓库单人开发一直直接提交 `main`；所有者已决定改为“单人开发例外”，见 ADR-0010 与 standards/02 §1。

## 已知问题与风险

- **Node 分发形态未定**：随包运行时目录或 Node SEA；Phase 6 前必须定稿，否则安装包体积与签名无解。
- **受信任边界**：B 路线下插件可直接用 Node 原生 fs/net/exec，shim 拦不住；只能在安装与运行界面明示“仅受信任插件”（ADR-0009）。
- **迟到注册**：`ready` 之后注册被拒绝并记诊断；真实扩展若有异步注册会丢工具，Phase 6 复核。
- **长会话与多插件并发**：本阶段未压测；Phase 6 做基准。
- **单机无 CI**：门禁全在本地跑（`gofmt`、`vet`、`-race`、`check-docs`）。

## 对下阶段的影响

- Phase 3 可直接消费 `eventbus` 与 `kernel`；插件诊断已是正式事件，UI 只需做面板。
- `kernel.Prompt` 仍是阻塞调用，Phase 3 在其外加 `Start`/`Wait` 异步门面，不改本阶段语义。
- Phase 6 接入真实 pi 扩展时只需补 shim 成员与分发形态，不再改协议或适配器骨架。

## 文档更新清单

- 新建：本报告、`handoff/H-006-phase-2-acceptance.md`。
- 更新：`docs/README.md`（登记本报告）、`docs/roadmap/00-roadmap.md`（看板与风险）、`docs/roadmap/phase-2-plugin-adapter.md`（状态）、`CHANGELOG.md`、`.agents/CONTEXT.md`。
