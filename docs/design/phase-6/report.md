# Phase 6 阶段进度报告（草案：基础设施与 shim 样本完成）

- 日期：2026-10-29
- 本阶段目标（一句话）：把真实 pi 编排扩展以受信任 Node 子进程接入，工具/命令/事件/steering/技能在桌面可用。

## 1. 已完成

- **适配器扩展**：`Options.NodePath`（Node 入口或直接 mock 可执行）、`SetSession`（切换会话不重启）、`SendHostEvent`、`Abort`、`HostUI`（select/confirm/input/editor）、`tool_update`、`shim` 版本校验。
- **管理器**：`internal/extensions` 发现用户/项目/开发目录，清单校验、SHA-256 信任、启停/撤销、工具与命令快照、事件扇出、通知、诊断、会话切换、进程清理。
- **设置 v2**：扩展路径/哈希/开关/信任时间、工作区信任；v1 读取迁移；损坏回退。
- **内核/bridge**：`kernel.Skills` 暴露用户/项目技能与诊断；bridge 注入 `ExtraTools`、`Steer`、技能查询、扩展/命令/信任回调；`TrustProject` 进入新会话。
- **桌面**：原生“扩展与技能”面板（信任/启停/诊断/技能/命令）；页面输入区 busy 时发送“补充指令”走 `Desktop.steer`；第四插件纳入默认注册。
- **shim 与样本**：`extensions/shim/host.mjs`（`locowrk-shim/1`：工具、命令、事件、UI、abort、心跳、关停）；`fixtures/extensions/orchestrator-sample` 注册工具/命令并发出并行子代理、workflow、通知。`TestNodeSampleEndToEnd` 通过（工具调用、命令、三类事件）。
- **原生对话框**：`desktop.DialogUI` 把 confirm/select 映射到 MyGo 消息框；input/editor 暂未实现，按取消处理（需补原生输入框）。
- **技能**：`TestSkillsTrustGate` 覆盖用户技能自动可用、项目技能未信任不加载、信任后加载。

## 2. 验收对照（roadmap/phase-6 §5）

| # | 场景 | 状态 | 说明 |
|---|---|---|---|
| 1 | 真实扩展加载 | 🟡 部分 | 管理器/信任/工具链路已通；真实 `todo`/`rpc-demo` 尚未构建验证，当前样本是自写 shim 扩展 |
| 2 | parallel 委派 | 🟡 部分 | `orchestrator-sample` 发出两个 spawn + settle 事件；UI 节点由现有编排树消费，未做真实子进程 |
| 3 | chain/async | 🟡 部分 | 命令与事件链路通；链式/异步取消未做样本 |
| 4 | steering | 🟡 部分 | bridge/kernel/UI 已接；未跑真实运行中补充指令冒烟 |
| 5 | UI 降级 | 🟡 部分 | shim 对 setWidget/setTitle/setStatus 发诊断、不抛错；confirm/select 已接原生消息框，input/editor 仍按取消处理 |
| 6 | 缓做项 | 🟡 部分 | `ctx.executeTool`/sendMessage 等在 shim 返回 not_implemented；未逐一验收 |
| 7 | 技能 | 🟡 部分 | `kernel.Skills` + `TestSkillsTrustGate` 通过；信任/撤销 UI 冒烟未做 |
| 8 | 故障 | 🟡 部分 | 管理器停止/重启路径、哈希失效测试通过；真实 Node 崩溃/超时未跑 |
| 9 | 回归 | ✅ 通过 | `go test -race ./...`、`npm run check`、`npm test`、`check-docs` 全绿；diff/plugin 冒烟通过 |

## 3. 读数

| 项 | 结果 |
|---|---|
| `TestNodeSampleEndToEnd` | 通过（约 0.4 s）；工具 `sample_echo`、命令 `sample_events`、两类事件 |
| `internal/extensions` 测试 | 发现/信任/哈希失效/项目管理通过 |
| 适配器/内核回归 | 全绿 |
| Node 运行时 | 开发用 PATH `node`；`runtime/node/` 未下载（ADR-0015 发布前置） |

## 4. 已知问题与下一步

1. **真实扩展样本**：下载/构建上游 `todo`、`rpc-demo`（固定提交，不入库），验证真实 `ExtensionAPI` 面；当前 shim 只覆盖样本使用的成员。
2. **原生输入框**：confirm/select 已接；input/editor 仍需原生单行/多行输入框，当前按取消处理。
3. **UI/技能**：面板的撤销信任、通知列表、命令参数输入；技能信任/撤销冒烟；页面“本会话已注入”提示。
4. **steering 冒烟**：真实运行中从页面补充指令，断言 `steering_applied` 与下一轮上下文。
5. **运行时**：准备 `runtime/node/` 与校验；发布缺运行时 fail-closed。
6. **真实故障**：Node 扩展崩溃、超时、进程树、工具冲突注入。

## 5. 体量与规范

- 新增 `internal/extensions` 约 700 行（含测试）；`extensions/shim` 约 300 行 JS；无新增 Go 依赖；Node 运行时未入库。
- 设置 v2 已写入 `desktop/settings.go`；旧 v1 迁移测试通过。
- 本报告为草案，Phase 6 未打 tag。
