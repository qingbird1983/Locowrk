# 工作上下文交接卡

- 更新：2026-10-30（会话主题：**Phase 6 收口**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3 / 4 / 5**：✅ 完成，tag 已打（`phase-5.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），规则见 `pi/FORK.md`。
- **Phase 5（WebView 混合渲染与 Diff 审阅）**：✅ 完成（tag `phase-5.end`；[报告](../docs/design/phase-5/report.md)）。
- **Phase 6（真实 Pi 编排插件与技能复用）**：✅ 完成（2026-10-30，tag `phase-6.end`；[报告](../docs/design/phase-6/report.md)）。
  - 九项验收全绿：真实 `todo`/`rpc-demo`、并行/链式/取消、steering、UI 降级、缓做诊断、技能信任、故障隔离、回归。
  - 关键能力：扩展管理器（信任/启停/工具/命令/事件/闸门/通知）、打包工具、随包 Node 运行时（v26.13.0 + 版本闸）、工具授权闸门与会话切换确认、进程树（作业对象）、发布 fail-closed。
- **界面范围（所有者 2026-10-28 决定）**：当前窗口是**功能粗稿**；布局密度、配色、按钮尺寸、图标、高 DPI、输入法观感在全部功能完成后统一做“界面 UI 重设计”，**不要在各阶段打磨界面**。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只保证可构建。

## 上一轮完成了什么

1. 补齐验收 2/3/6：并行委派完成顺序（`Seq` 递增）、链式/异步/取消、全部缓做成员诊断；新增 `fixtures` 命令与三条测试。
2. 收口：报告定稿（读数、体量核对、规范复查、下阶段影响），看板 Phase 6 标完成，CHANGELOG 0.11.0，CONTEXT 重写，handoff 滚动，打 tag `phase-6.end`。
3. 体量：`manager.go` 拆为 5 个文件；`cmd/locowrk` 拆出 `smoke.go`；`kernel` 拆出 `gate.go`；`measure` 夹具登记豁免。

## 下一步（按优先级）

1. **所有者决策**：先做“界面 UI 重设计”，还是先做 Phase 7（子进程桌面插件、UI 描述模式）探索。
2. **UI 重设计**：扩展/技能面板、标签栏、Diff 页仍是功能粗稿；数据接口已稳定（`StatusSource`、`Extensions`/`Skills`/`Commands`）。
3. **发布工程**：安装包、签名、钥匙串、`runtime/node/` 与 CI 打包（Phase 4~7 并行项，仍暂缓）。

## 未决问题

- **会话条目**：shim 的 `ctx.sessionManager` 无真实条目（`getBranch` 返回空），依赖历史重建的扩展状态重启后从头开始。
- **阻断粒度**：`tool_call` 只阻断单次调用；`Terminate`（整批）与页面内切换会话的确认未接。
- **面板**：撤销信任/通知/命令参数只有数据层与回调测试，按钮级冒烟未做。
- **平台**：macOS/Linux 只保证构建；WebView 焦点/IME/高 DPI 与进程组清理差异未在真机验证。
- **隐藏验收边界**：Windows 隐藏窗口不构建原生内容、页面 rAF 暂停；渲染用 `ui.NewTester`，crash 用 `-show`（P-008）。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理，不是沙箱。

## 关键提醒

- **窗口类型**（P-007）：原生窗口没有 `win.Page()`；用 `web.Page()` + `Event.EmitPage`。
- **隐藏窗口**（P-008）：不画原生内容、页面 rAF 暂停；不要用隐藏冒烟断言渲染。
- **WebView 生命周期**：只在绑定 goroutine/按钮回调/冒烟里创建销毁；**绝不在 `Build` 中**，也不得持宿主主锁创建/销毁（主线程死锁）。
- **WebView 测试**（P-009）：`ui.NativeWebView` 含 internal 类型，仓库外不能实现假接口；用零值 `*mygo.WebView` 句柄 + 注入工厂。
- **插件纪律**：状态只存数据，`Build` 每帧重建，`ui.Element` 不落状态，不 import `internal/kernel`；事件回调只改数据 + `Invalidate()`。
- **设置文件**：`%AppData%\Locowrk\settings.json`；`-config` 覆盖，冒烟/测试必须指到临时目录。
- **前端构建顺序**：改 `web/src` 后先 `npm run build`，再 `go build`（`dist/` 被 embed 且入库）；契约变更后 `go run ./cmd/locowrk -generate-bindings web/src/mygo.ts`，`git diff` 只应有预期字段。
- **页面以事件为准**：系统消息不渲染；工具结果只进卡片；消息 ID 不要改回 `role+timestamp`。
- **冒烟隔离**：真实 key 只走环境变量；`HOME`/`USERPROFILE` 指到临时目录；`-smoke` 默认临时 cwd。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...`、`cd web && npm run check && npm run build`、`bash scripts/check-docs.sh`。Windows 上 `-race` 需要 `C:\msys64\mingw64\bin` 在 PATH。
- **PowerShell 改文件会毁中文**：用编辑工具（P-006，已两次踩坑）。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG。
- **导入边界**：正式代码只有 `internal/kernel` 引用 `pi/...`；MyGo 只在 `desktop/`、`cmd/locowrk`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100、代码单文件 ≤500（测试 ≤800）、CONTEXT ≤150、handoff 留 5 份。
- **运行时**：`node scripts/fetch-node-runtime.mjs` 拉取钉死版本到 `runtime/node/`（不入库）；`--check` 只校验；开发覆盖用 `LOCOWRK_NODE`/`-node`（不校验版本）。发布判定是 `mygo.IsDev()` 为假。
- **扩展打包**：`cd tools/extension-build && npm install` 后 `node build.mjs --src <源码> --out <包>`；`--check` 校验；6 个夹具都要通过 `--check`。
- **真实样本**：不入库，本机在 `%TEMP%\locowrk-samples\`；`LOCOWRK_REAL_SAMPLE_DIR=<packages> go test ./internal/extensions -run TestRealPiSamples`。
- **故障夹具**：`fixtures/faults/{crash,slow,tool-conflict,grandchild,throw}`；`timeoutMs` 1000~600000（P-012）；子进程不得写 stdout（P-013）。
- **代理**：本机 `127.0.0.1:7897`（HTTP）；`curl -x http://127.0.0.1:7897` 可访问 GitHub。
- 踩坑索引：`P-001`~`P-013`；方法：`skills/S-001-runtime-comparison-spike.md`。
