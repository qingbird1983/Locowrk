# Phase 6 阶段进度报告（定稿）

- 日期：2026-10-30
- 状态：定稿（tag `phase-6.end`）
- 本阶段目标（一句话）：把真实 pi 编排扩展以受信任 Node 子进程接入，工具/命令/事件/steering/技能在桌面可用。

## 1. 交付物

- **扩展管理器** `internal/extensions`：发现用户/项目/开发目录；SHA-256 信任与撤销；启停与失败隔离；工具/命令快照；事件扇出与授权闸门；通知与诊断；会话切换；进程树清理。已按体量拆为 `manager.go` / `manager_registry.go` / `manager_gate.go` / `manager_trust.go` / `node_runtime.go`。
- **适配器**：`NodePath`、`SetSession`、`SendHostEvent`、`RequestEvent`（`wantsEventAck`）、`Abort`、`HostUI`（select/confirm/input/editor）、`tool_update`、shim 与版本校验、超时与崩溃诊断、作业对象进程树。
- **shim** `extensions/shim/host.mjs`（`locowrk-shim/1`）：工具、命令、事件回执、UI、abort、心跳、关停；`compat/` 提供 `@earendil-works/pi-ai`、`pi-tui`、`typebox` 的最小实现。
- **打包与运行时**：`tools/extension-build`（esbuild、可复现、`--check`）；`scripts/fetch-node-runtime.mjs` + `runtime/node/`（v26.13.0，Windows x64，SHA-256 `dcc60d59…b59150a`）。
- **内核/bridge/桌面**：`kernel.Skills`、`kernel.BeforeTool`（工具闸门）、`Steer` 空闲报错；bridge 注入工具/闸门/技能/信任；原生“扩展与技能”面板；输入区补充指令；新建/打开会话的扩展确认；发布模式缺运行时 fail-closed。
- **夹具**：`fixtures/extensions/orchestrator-sample`（工具/命令/并行/链式/取消/延迟成员/闸门）；`fixtures/faults/{crash,slow,tool-conflict,grandchild,throw}`。
- **文档**：`design/phase-6/{runtime,extensions,skills}.md`、ADR-0015、本报告。

## 2. 验收对照（roadmap/phase-6 §5）

| # | 场景 | 状态 | 证据 |
|---|---|---|---|
| 1 | 真实扩展加载 | ✅ | 上游 `todo`/`rpc-demo`（`earendil-works/pi@38ff9917`）打包后加载：todo 1 工具 + 1 命令并 list→add→list；rpc-demo 3 命令；未信任不启动由既有测试覆盖 |
| 2 | parallel 委派 | ✅ | `TestParallelDelegationSettlesInCompletionOrder`：两个节点先后出现，完成者先 settle，`Seq` 严格递增；桌面树/workflow 消费由 plugin 冒烟覆盖 |
| 3 | chain/async | ✅ | `TestChainStepsAsyncNotifyAndCancel`：三步 running/done 顺序正确、异步完成通知；`AbortAll` 后停止继续推进并发出 skipped(cancelled) |
| 4 | steering | ✅ | 冒烟 `steer`（脚本模型）与 `steer-real`（DeepSeek）：`steering_applied`、下一轮历史含补充文本、空闲补充返回明确错误 |
| 5 | UI 降级 | ✅ | rpc-demo：setTitle/setWidget/setStatus 各一条诊断；input/editor 弹宿主对话框；危险 bash 触发 select 并阻断；取消会话切换生效 |
| 6 | 缓做项 | ✅ | `TestDeferredMembersReportDiagnostics`：executeTool/sendMessage/sendUserMessage/appendEntry/compact/setWidget/setEditorText/pasteToEditor/custom/registerShortcut 各有诊断、不抛错，之后工具仍可用 |
| 7 | 技能 | ✅ | `TestSkillsTrustRoundTrip`（面板同一调用链）：未信任只列用户技能 → 信任后出现项目技能 → 撤销后隐藏；`TestSkillsTrustGate` 覆盖内核 |
| 8 | 故障 | ✅ | 崩溃/孙进程/工厂抛异常/超时/工具重名/哈希不符：状态 failed + 诊断，兄弟扩展不受影响，工具下线 |
| 9 | 回归 | ✅ | `go test -race ./...`、`npm run check/build/test`、tools `npm test`、6 个夹具 `--check`、`check-docs`、faux/tool/cancel/plugin/diff/steer 冒烟全绿 |

## 3. 读数

| 项 | 结果 |
|---|---|
| 隐藏窗口冒烟 | `steer` 约 8.2 s；`steer-real`（DeepSeek）约 3.0 s、4 条消息；faux/tool/cancel/plugin/diff 全部通过 |
| 真实样本 | todo bundle 20.4 KB、rpc-demo 12.4 KB（自包含 shim）；`TestRealPiSamples` 约 0.4–0.6 s |
| 并行/链式 | 并行 0.25 s；链式 + 取消 0.65 s |
| 故障 | 崩溃 0.8 s、孙进程 1.9 s、工厂异常 0.3 s、超时 1.2 s、重名 0.5 s |
| 打包工具 | 4 项自测；同输入同字节；`--check` 可检出篡改；6 个夹具一致 |
| 运行时版本闸 | 随包 v26.13.0 通过；不匹配时整条 A 轨禁用 |
| 发布模式 | 缺 `runtime/node` 时打印提示、A 轨禁用、桌面可用；有运行时时无提示 |

## 4. 体量核对（standards/08 §7）

- **docs/**：62 篇（软上限 120）；本阶段设计稿 4 篇（+ADR-0015）；单篇最大 < 300 行；无孤儿文档（`check-docs` 通过）。
- **.agents/**：21 个文件（上限 40）；CONTEXT 78 行（≤150）；handoff 滚动 5 份；pitfalls 13 篇；skills 1 篇。
- **代码**：生产代码单文件最大 `web/src/transcript.ts` 500 行；`manager.go` 已拆为 5 个文件（最大 362 行）；测试 `kernel_test.go` 614 行（≤800）；**豁免**：`fixtures/spike-runtime/cmd/measure/main.go` 614 行（Phase 2 测量夹具，非发布代码，文件头已注明）。
- **依赖**：Go 无新增；npm 新增 1 个开发依赖 `esbuild 0.28.2`（仅 `tools/extension-build`，运行期不使用），锁文件入库。
- **前端资产**：无新增；`dist/` 合计约 100 KB，单文件 < 1 MB。
- **豁免登记**：`measure`（理由如上）；`pi/` 为 ADR-0012 的 MIT fork，按 vendored 代码不计入本仓库预算。

## 5. 已知问题与风险

- **平台**：macOS/Linux 只验证构建，未运行；WebView 与进程组清理的平台差异按 ADR-0007/0015 记录。
- **会话条目**：shim 的 `ctx.sessionManager` 无真实条目（`getBranch` 返回空），依赖历史重建的扩展状态在重启后从头开始。
- **阻断粒度**：`tool_call` 只阻断单次调用，`Terminate`（终止整批）未使用；`session_before_switch` 只在新建/打开会话触发。
- **面板**：按钮级冒烟未做，覆盖到回调与数据层；“本会话已注入”提示留 UI 重设计。
- **信任边界**：A 轨扩展是受信任代码，能力清单是契约不是沙箱；界面已明示。
- **真实样本不进 CI**：需要代理/网络与 Node，`TestRealPiSamples` 默认跳过，结果人工记录。
- **隐藏窗口**：不构建原生内容、页面 rAF 暂停；渲染相关断言用可见窗口（P-008）。

## 6. 规范复查（standards/09 四问）

| 规范 | 漏洞 | 过严 | 过松 | 冗余/矛盾 |
|---|---|---|---|---|
| 01 仓库结构 | 无发现 | 无发现 | 无发现 | 无发现 |
| 02 Git 流程 | 无发现 | 无发现 | 无发现 | 无发现 |
| 03 文档 | 报告“草案/定稿”状态需要人工维护；已在本篇对齐 | 无发现 | 无发现 | 无发现 |
| 04 代码 | 无发现 | 单文件 500 行对大型测试偏紧，已按 800 行放宽 | 无发现 | 无发现 |
| 05 测试 | 真实样本无法进 CI，只能人工记录；已固定命令与触发变量 | 无发现 | 无发现 | 无发现 |
| 06 安全 | 无发现 | 无发现 | 扩展能力是契约而非沙箱，已在面板与报告明示 | 无发现 |
| 07 AI 协作 | 无发现 | 无发现 | 无发现 | 无发现 |
| 08 体量 | 无发现（已拆分 manager） | 无发现 | 无发现 | `dist/` 入库与“禁止构建产物”仍冲突，沿用 Phase 5 决定留待发布阶段 |
| 09 复查 | 无发现 | 无发现 | 无发现 | 无发现 |

处置：本轮只做说明级补充（measure 豁免、dist 冲突沿用既有登记），不改规范条文。

## 7. 对下阶段的影响

- Phase 7（子进程桌面插件）可复用扩展管理器、事件闸门、打包工具与故障夹具；UI 描述模式需要新协议，不建议直接扩展 `locowrk-shim/1`。
- UI 重设计可基于现有面板数据（扩展/技能/命令/通知）重排，不需要改管理器接口。
- 发布工程需把 `runtime/node/`、夹具 `--check`、扩展签名与安装包纳入流水线。

## 8. 文档更新清单

- `design/phase-6/report.md`（本报告，定稿）
- `docs/roadmap/00-roadmap.md`（Phase 6 → 完成）
- `CHANGELOG.md`（0.11.0）
- `.agents/CONTEXT.md`、`.agents/handoff/H-027-phase-6-close.md`
- `docs/README.md` 无需新增条目（报告已登记）
