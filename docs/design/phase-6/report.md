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
- **steering（2026-10-29 补）**：内核 `Steer` 在空闲回合返回 `ErrNotRunning`，桌面转成中文提示；页面冒烟 `steer`/`steer-real` 覆盖“运行中补充 → `steering_applied` → 下一轮请求带上补充文本 → 空闲调用报错”。
- **随包运行时（2026-10-29 补）**：`scripts/fetch-node-runtime.mjs` 下载钉死版本、校验 SHA-256 后只保留 `node.exe` 与许可证；管理启动前核对版本，不一致整条 A 轨禁用并写明原因。
- **打包工具（2026-10-30 补）**：`tools/extension-build/` 用 esbuild 把源码 + shim 打成单文件 `bundle.mjs`，生成 `plugin.json`/`build.json`；拒绝 `.node` 原生模块、远程 import 与无 scope 能力；同输入同字节；`--check` 供 CI 使用。`TestBuiltSampleEndToEnd` 用构建产物跑通管理器链路。
- **真实样本（2026-10-30 补）**：经本机代理取回上游 `earendil-works/pi@38ff99177fba21e9c57a670f4e95adcf407c99c7` 的 `todo.ts`（SHA-256 `b6e4d370…db57166`）与 `rpc-demo.ts`（`3d54a396…29ae640d`），用打包工具构建后由 `TestRealPiSamples`（`LOCOWRK_REAL_SAMPLE_DIR` 触发，默认跳过）跑通：工具/命令、UI no-op 诊断、输入/编辑对话框与通知。
- **为真实样本补齐的兼容面**：`extensions/shim/compat/`（`@earendil-works/pi-ai`、`@earendil-works/pi-tui`、`typebox` 的最小实现，随包内联）；`registerCommand(name, options)` 双签名；事件处理器第二参数 `ctx`；`ctx.sessionManager.getBranch/getEntries` 返回空（见已知限制）；命令名允许连字符；被拒绝的插件通知记为诊断（此前静默丢弃）。
- **事件闸门（2026-10-30 补）**：shim 在 `initialize` 声明 `wantsEventAck`，宿主把 `tool_call` 与 `session_before_switch` 作为请求发送并等待回执（`block/reason`、`cancel`）。内核新增 `BeforeToolFunc`（映射 pi 的 `BeforeToolCall`），bridge 透传，`cmd/locowrk` 接 `Manager.GateToolCall`；新建/打开会话前走 `Manager.ConfirmSessionSwitch`。插件无响应时 fail-open 并记通知。
- **故障与技能（2026-10-30 补）**：新增 `fixtures/faults/`（崩溃、超时、工具重名）与三条测试；管理器状态行跟随进程状态（崩溃后显示 failed 与原因），工具超时记 `timeout` 诊断；技能信任链路由 `TestSkillsTrustRoundTrip` 覆盖，撤销后新会话不再注入项目技能。

## 2. 验收对照（roadmap/phase-6 §5）

| # | 场景 | 状态 | 说明 |
|---|---|---|---|
| 1 | 真实扩展加载 | ✅ 通过 | 上游 `todo` 与 `rpc-demo` 经 `tools/extension-build` 打包后由管理器加载：`todo` 注册 1 工具 + 1 命令并执行 list/add/list，`/todos` 命令按 no-op 提示；`rpc-demo` 注册 3 命令；未信任不启动由既有测试覆盖 |
| 2 | parallel 委派 | 🟡 部分 | `orchestrator-sample` 发出两个 spawn + settle 事件；UI 节点由现有编排树消费，未做真实子进程 |
| 3 | chain/async | 🟡 部分 | 命令与事件链路通；链式/异步取消未做样本 |
| 4 | steering | ✅ 通过 | 冒烟 `steer`（脚本模型）与 `steer-real`（真实 DeepSeek）通过：`steering_applied` 出现、下一轮历史含补充文本、空闲补充返回明确错误；内核新增 `ErrNotRunning` |
| 5 | UI 降级 | ✅ 通过 | `rpc-demo` 实测：`setTitle`/`setWidget`/`setStatus` 各一条诊断且扩展继续；`rpc-input`/`rpc-editor` 弹宿主对话框并回写通知；`select` 由危险 `bash` 调用触发并阻断该调用；`confirm` 由新建/切换会话触发，取消即中止切换 |
| 6 | 缓做项 | 🟡 部分 | `ctx.executeTool`/sendMessage 等在 shim 返回 not_implemented；未逐一验收 |
| 7 | 技能 | ✅ 通过 | 用户技能自动可用；项目技能未信任不加载、信任后出现在列表、撤销后不再注入新会话；`TestSkillsTrustRoundTrip` 覆盖面板同一条调用链（`Skills`/`SetProjectTrust`），`TestSkillsTrustGate` 覆盖内核信任闸 |
| 8 | 故障 | 🟡 部分 | 崩溃：进程退出后状态转 failed、记 `crashed` 诊断、兄弟扩展不受影响（`TestCrashedExtensionIsDisabledAndIsolated`）；超时：工具按 `timeoutMs` 超时并记 `timeout` 诊断、进程仍可用（`TestToolTimeoutRecordsDiagnostic`）；工具冲突只禁用后注册方（`TestToolConflictDisablesOnlyTheSecond`）；哈希不符见既有测试；**崩溃时的孙进程清理与“抛异常禁用”尚未单独注入** |
| 9 | 回归 | ✅ 通过 | `go test -race ./...`、`npm run check`、`npm test`、`check-docs` 全绿；diff/plugin 冒烟通过 |

## 3. 读数

| 项 | 结果 |
|---|---|
| `TestNodeSampleEndToEnd` | 通过（约 0.4 s）；工具 `sample_echo`、命令 `sample_events`、两类事件 |
| steering 冒烟 `steer`（脚本模型） | 通过；约 8.2 s，`attempts=2`（首帧竞态被重试），空闲补充报“没有正在运行的回合” |
| steering 冒烟 `steer-real`（DeepSeek） | 通过；约 4.0 s、4 条消息；同一断言集 |
| `TestSteeringRequiresRunningTurnAndReachesNextRequest` | 通过；空闲返回 `ErrNotRunning`、运行中入队并产生 `steering_applied`、历史含补充文本 |
| `tools/extension-build` 自测（`npm test`） | 4 项通过：打包产物、可复现（同字节）、`--check` 检出篡改、拒绝原生模块/远程 import/无 scope 能力 |
| `TestBuiltSampleEndToEnd` | 通过（约 0.3 s）：构建产物被管理器启动并执行 `sample_echo` |
| `TestRealPiSamples`（真实上游样本） | 通过（约 0.5 s）：`todo` 工具 list→add→list 与 `/todos` 命令；`rpc-demo` 3 命令、`ui.setTitle`/`ui.setWidget`/`ui.setStatus` 诊断、input/editor 对话框与 3 条通知；危险 `bash` 调用被 select 对话框阻断；`session_before_switch` 取消可中止切换 |
| `TestToolCallGate` / `TestBeforeToolGateBlocksExecution` | 通过：事件请求回执合并为 `block/reason`；内核 `BeforeTool` 阻断后模型收到错误工具结果并继续回合 |
| 故障夹具（`fixtures/faults/`） | 崩溃：状态转 failed + `crashed` 诊断，兄弟工具仍可用（约 0.4 s）；超时：`timeoutMs=1000` 生效并记 `timeout`，超时后进程仍可用（约 1.2 s）；工具重名：后注册方 failed，原因 `already provided by orchestrator-sample`（约 0.4 s） |
| `TestSkillsTrustRoundTrip` | 通过：未信任只列用户技能；信任后出现项目技能；撤销后隐藏（同一 `Skills`/`SetProjectTrust` 调用链） |
| 真实样本包体积 | `todo` bundle 19,327 B；`rpc-demo` 11,333 B（均自包含 shim） |
| `internal/extensions` 测试 | 发现/信任/哈希失效/项目管理/运行时版本闸通过 |
| 适配器/内核回归 | 全绿 |
| Node 运行时 | 已落到 `runtime/node/`（v26.13.0，Windows x64，SHA-256 `dcc60d59…b59150a`）；开发可用 `LOCOWRK_NODE` 或 PATH 覆盖（不校验版本） |

## 4. 已知问题与下一步

1. **真实扩展样本**：✅ 本轮完成（经代理取回并打包，`TestRealPiSamples` 通过）；样本源码不入库，复现步骤见本报告 §3 与 `tools/extension-build/README.md`。
2. **原生输入框**：input/editor 已能弹宿主对话框（`rpc-demo` 验证）；原生控件仍是功能粗稿，视觉效果留 UI 重设计。
3. **UI/技能**：面板的撤销信任、通知列表、命令参数输入；技能信任/撤销冒烟；页面“本会话已注入”提示。
4. **shim 限制**：`ctx.sessionManager` 没有真实会话条目（`getBranch` 返回空），依赖历史重建的扩展状态会丢；`session_before_switch` 已接新建/打开会话，但页面内切换标签等尚无触发点。
5. **运行时**：`runtime/node/` 已就绪并做版本闸；仍需把“缺运行时”接进发布构建的 fail-closed 检查（当前开发回退仅在开发模式生效）。
6. **故障**：仍有崩溃时的孙进程清理（现有测试只覆盖主动停止）与“扩展抛异常即禁用”未单独注入。

## 5. 体量与规范

- 新增 `internal/extensions` 约 950 行（含测试）；`extensions/shim` 约 350 行 JS（含 `compat/` 三个模块）；`scripts/fetch-node-runtime.mjs` 约 200 行；`tools/extension-build` 约 300 行 JS（含测试），新增 esbuild 0.28.2 开发依赖（仅打包工具，不在运行期）；Node 运行时未入库（`runtime/node/` 已忽略），真实样本源码未入库。
- 设置 v2 已写入 `desktop/settings.go`；旧 v1 迁移测试通过。
- 本报告为草案，Phase 6 未打 tag。
