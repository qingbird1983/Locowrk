# 工作上下文交接卡

- 更新：2026-10-30（会话主题：**Phase 6 进程树与发布 fail-closed**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3 / 4**：✅ 完成，tag 已打（`phase-4.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），规则见 `pi/FORK.md`。
- **Phase 5（WebView 混合渲染与 Diff 审阅）**：✅ 完成（2026-10-28，tag `phase-5.end`；[报告](../docs/design/phase-5/report.md)）。
  - 标签页宿主 + Diff 页面；10k 行 0.43 s；工具卡片链路、切标签状态保持、关闭释放均验收。
  - ADR-0014；设计 `docs/design/phase-5/webview.md`、`diff.md`；报告定稿。
- **Phase 6（真实 Pi 编排插件与技能复用）**：🟡 基础设施落地（2026-10-29），未完成验收、未打 tag。
  - 已落地：`internal/extensions`（发现/信任/启停/工具/命令/事件/通知）、设置 v2、adapter NodePath/SetSession/UI/tool_update、`kernel.Skills`、bridge steer/tools/trust、原生“扩展与技能”面板、页面补充指令。
  - **2026-10-29 补**：steering 验收通过（内核 `ErrNotRunning`、冒烟 `steer`/`steer-real`，真实 DeepSeek 通过）；`runtime/node/` v26.13.0 就位 + 启动版本闸（不一致整条 A 轨禁用）。
  - **2026-10-30 补**：`tools/extension-build/` 打包工具（单文件 bundle + `plugin.json`/`build.json`、可复现、`--check`、拒绝原生/远程导入），夹具改为打包产物。
  - **2026-10-30 再补**：真实上游样本 `todo`/`rpc-demo`（`earendil-works/pi@38ff9917`，不入库）打包后跑通；补 `extensions/shim/compat/`、命令双签名、事件 `ctx`、命令名连字符、通知被拒诊断。
  - **2026-10-30 三补**：事件闸门（`wantsEventAck`、`tool_call` 阻断、`session_before_switch` 取消）与内核 `BeforeTool` 钩子；真实 `rpc-demo` 验证 select/confirm。
  - **2026-10-30 四补**：故障隔离（崩溃/超时/工具重名/孙进程/工厂异常夹具与测试，状态行跟随进程状态）与技能信任链路（未信任/信任/撤销）；Windows 作业对象托管进程树，崩溃时孙进程一起结束。
  - **2026-10-30 五补**：发布模式缺 `runtime/node` 时 A 轨 fail-closed 并打印提示；工厂抛异常即禁用；`--check` 覆盖 6 个夹具。
  - shim `extensions/shim/host.mjs` + `fixtures/extensions/orchestrator-sample` 端到端通过（工具/命令/事件）。
  - **未完成**：面板按钮级冒烟、会话条目回放、阻断后的批终止、Phase 6 报告定稿与 tag。
- **界面范围（所有者 2026-10-28 决定）**：当前窗口是**功能粗稿**；布局密度、配色、按钮尺寸、图标、高 DPI、输入法观感在全部功能完成后统一做“界面 UI 重设计”，**不要在各阶段打磨界面**。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只保证可构建。

## 上一轮完成了什么

1. 进程树：Windows 作业对象（Kill-on-close）托管扩展及其子进程，崩溃/停止时整棵树结束；`TestCrashKillsGrandchildren` 用心跳文件验证。
2. 工厂异常：`factory_error` 诊断即禁用扩展（状态 failed、工具/命令下线、诊断保留）。
3. 发布 fail-closed：`MYGO_ENV=production` 且缺 `runtime/node` 时打印提示并禁用 A 轨，桌面照常可用（临时目录二进制实测）。
4. 修复两个回归：夹具孙进程曾污染协议 stdout（P-013）；`TestHashChangeLosesTrust` 因进程未及时退出而锁文件（作业终止后等待进程退出）。
5. 文档：CHANGELOG 0.10.8、报告验收 8/9 全绿、CONTEXT、H-026、P-013，P-006 补记两次 PowerShell 事故。

## 下一步（按优先级）

1. **Phase 6 收尾**：面板按钮级冒烟、会话条目回放（`sessionManager.getBranch`）、阻断后的批终止（`Terminate`）——完成后写定稿报告并打 `phase-6.end`。
2. **UI 重设计**：功能完成后单独立项；当前扩展面板/标签栏/Diff 页仍是功能粗稿。
3. **发布工程**：仍暂缓；Node 运行时与签名纳入发布阶段。

## 未决问题

- **双 WebView 未实测**：焦点、IME、高 DPI、销毁后进程回收；spike 是硬前置。
- **标签页内存**：4 个上限；若单页常驻过高，改为销毁非活动标签（会丢状态，需重评验收 3）。
- **Diff 文本选择**：虚拟列表只能选已挂载行，跨屏复制不承诺。
- **插件状态**：会话切换即清空，切回旧会话不恢复树/时间线；Phase 6 接真实数据源时评估回放。
- **隐藏验收边界**：Windows 隐藏窗口不构建原生内容、页面 rAF 暂停；数据用隐藏冒烟，渲染用 `ui.NewTester`，crash 用 `-show`（P-008）。
- **panic 恢复**：丢一帧、当帧构建计数偏移；测试未出现输入丢失，暂不做双帧预检。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。
- **会话条目**：shim 的 `ctx.sessionManager` 没有真实条目（`getBranch` 返回空）；依赖历史重建状态的扩展（如 todo 重建）在重启后从头开始。
- **事件闸门**：插件在 `initialize` 声明 `wantsEventAck` 后，宿主把 `tool_call`/`session_before_switch` 当请求发出；无响应 fail-open 并记通知。`tool_call` 失败不得中断回合。

## 关键提醒

- **窗口类型**（P-007）：原生窗口没有 `win.Page()`；用 `web.Page()` + `Event.EmitPage`。
- **隐藏窗口**（P-008）：不画原生内容、页面 rAF 暂停；不要用隐藏冒烟断言渲染。
- **WebView 生命周期**：只在绑定 goroutine/按钮回调/冒烟里创建销毁；**绝不在 `Build` 中**，也不得持宿主主锁创建/销毁（主线程死锁）。
- **WebView 测试**（P-009）：`ui.NativeWebView` 含 internal 类型，仓库外不能实现假接口；用零值 `*mygo.WebView` 句柄 + 注入工厂，`CapturePage` 不含子 WebView。
- **插件纪律**：状态只存数据，`Build` 每帧重建，`ui.Element` 不落状态，不 import `internal/kernel`；事件回调只改数据 + `Invalidate()`。
- **设置文件**：`%AppData%\Locowrk\settings.json`；`-config` 覆盖，冒烟/测试必须指到临时目录。
- **前端构建顺序**：改 `web/src` 后先 `npm run build`，再 `go build`（`dist/` 被 embed 且入库）；契约变更后 `go run ./cmd/locowrk -generate-bindings web/src/mygo.ts`，`git diff` 只应有预期字段。
- **页面以事件为准**：系统消息不渲染；工具结果只进卡片；消息 ID 不要改回 `role+timestamp`。
- **冒烟隔离**：真实 key 只走环境变量；`HOME`/`USERPROFILE` 指到临时目录；`-smoke` 默认临时 cwd。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...`、`cd web && npm run check && npm run build`、`bash scripts/check-docs.sh`。
- **PowerShell 改文件会毁中文**：用编辑工具（P-006）。
- **运行时**：`node scripts/fetch-node-runtime.mjs` 拉取钉死版本到 `runtime/node/`（不入库）；`node ... --check` 只校验。GitHub 与 nodejs.org 直连不可靠，脚本按镜像顺序尝试并用发布方校验值核对（P-010）。开发覆盖用 `LOCOWRK_NODE`/`-node`，此时不校验版本。
- **扩展打包**：`cd tools/extension-build && npm install` 后 `node build.mjs --src <源码目录> --out <包目录>`；`--check` 校验产物与源码一致；`node_modules` 不入库，运行期不构建。
- **真实样本**：源码不入库；本机已取在 `%TEMP%\locowrk-samples\`（`todo.ts`/`rpc-demo.ts` + `src/` + `packages/`）。重跑：`$env:LOCOWRK_REAL_SAMPLE_DIR='<packages 目录>'; go test ./internal/extensions -run TestRealPiSamples -v`。
- **故障夹具**：`fixtures/faults/{crash,slow,tool-conflict,grandchild,throw}`；`timeoutMs` 合法范围 1000~600000，越界会被拒（现在会落诊断，P-012）。
- **进程树**：Windows 用作业对象（Kill-on-close）托管，崩溃即清孙进程；扩展子进程**不得写 stdout**（协议通道，P-013）。
- **发布判定**：`mygo.IsDev()` 为假（`MYGO_ENV=production` 或 `-ldflags`）即发布模式；缺 `runtime/node` 时 A 轨禁用并打印提示。
- **代理**：本机 `127.0.0.1:7897`（HTTP）；curl 加 `-x http://127.0.0.1:7897` 可访问 GitHub（P-010）。
- **steering 冒烟**：`go run ./cmd/locowrk -smoke -smoke-scenario steer`（脚本模型）或 `-smoke-scenario steer-real`（需 `DEEPSEEK_API_KEY`）。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG。
- **导入边界**：正式代码只有 `internal/kernel` 引用 `pi/...`；MyGo 只在 `desktop/`、`cmd/locowrk`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100、代码单文件 ≤500、CONTEXT ≤150、handoff 留 5 份。
- 踩坑索引：`P-001`~`P-013`；方法：`skills/S-001-runtime-comparison-spike.md`。
