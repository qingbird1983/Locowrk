# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 4 实现与自动验收**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3**：✅ 完成，tag 已打（`phase-3.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），规则见 `pi/FORK.md`。
- **Phase 4（PluginHost 与原生插件）**：🟡 **实现完成，自动验收绿，待人工确认**。
  - 主窗口：“原生外框 + 内嵌 WebView”（ADR-0013）；页面原样，右栏/顶条为原生。
  - 三个插件：编排树（右栏上）、workflow 时间线（顶条）、工具检查器（右栏下）；工具卡片点击联动。
  - 启停与右栏状态存 `os.UserConfigDir()/Locowrk/settings.json`；单插件 panic 只停用它自己。
  - 报告：`docs/design/phase-4/report.md`（**草稿**，人工项未确认）。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只保证可构建。
- **看板唯一来源**：`docs/roadmap/00-roadmap.md`；流水唯一来源：`CHANGELOG.md`。

## 上一轮完成了什么

1. 窗口迁移：`Content: ui.View(host.View)` + `NewWebView("/")`；`App.SetPage` + `Event.EmitPage` 恢复事件流；`-config`、`-show`。
2. 宿主与外壳：`desktop/plugin.go|host.go|settings.go|shell.go`；按 Kind 分发、会话切换清空、插槽级 recover、设置原子写、`Snapshot`（含 `frames`）。
3. 三个插件与联动：`plugins_tree.go`、`plugins_timeline.go`、`plugins_tools.go`；`Desktop.SelectToolCall`（前端只加这一个绑定）。
4. 前端适配：rAF + 120 ms 定时器兜底（P-008）、窄宽度媒体查询、`dist` 重建、生成客户端重跑。
5. 验收：`go test -race ./...` 全绿；100 节点 + 500 调用 0.70 ms/帧；`faux/tool/cancel/long/restore/plugin/plugin-crash` 通过；`real` 未跑（无 key）。

## 下一步（按优先级）

1. **人工确认**：可见窗口的栏宽/分隔线/高 DPI、原生栏 ↔ 页面的焦点与中文输入法；把报告转定稿、看板改完成、打 `phase-4.end`。
2. **补 `real` 冒烟**：有 key 时 `-smoke -smoke-scenario real`，断言不放宽。
3. **Phase 5 设计稿**：Diff 审阅插件（`session.tab` 首次启用），先写 `docs/design/phase-5/`。
4. **规范复查项**：`standards/08 §6`“禁止提交 dist/”与前端 embed 入库冲突，留发布工程一并处理。

## 未决问题

- **可见窗口观感未确认**：栏宽 260、1px 分隔线、顶条留白、高 DPI 均需人眼；中文输入法跨原生/页面同理。
- **隐藏验收边界**：Windows 隐藏窗口不构建原生内容，也不跑页面 rAF；`plugin-crash` 会显示窗口。不要把隐藏冒烟的耗时当交互延迟。
- **里程碑**：本轮未单独打 `phase-4.start`（设计/实现同轮推进），收尾只打 `phase-4.end`。
- **插件状态**：只从后续事件重建；切回旧会话不恢复树/时间线，随 Phase 6 数据源再评估。
- **panic 恢复**：丢一帧、当帧构建计数偏移；测试未出现输入丢失，暂不做双帧预检。
- **长会话内存**：5k 约数 MB，10k+ 未测；`pi/` 约 2.6 MB / 132 文件（ADR-0012 豁免）。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。
- **Windows 之外**：MyGo/WebView 只在 Windows 冒烟。

## 关键提醒

- **窗口类型**（P-007）：原生窗口没有 `win.Page()`；用 `web.Page()` + `Event.EmitPage`。
- **隐藏窗口**（P-008）：不画原生内容、页面 rAF 暂停；数据用隐藏冒烟，渲染用 `ui.NewTester`，crash 用 `-show`。
- **插件纪律**：状态只存数据，`Build` 每帧重建，`ui.Element` 不落状态，不 import `internal/kernel`；事件回调只改数据 + `Invalidate()`。
- **设置文件**：`%AppData%\Locowrk\settings.json`；`-config` 覆盖，冒烟/测试必须指到临时目录。
- **前端构建顺序**：改 `web/src` 后先 `npm run build`，再 `go build`（`dist/` 被 embed 且入库）；契约变更后 `go run ./cmd/locowrk -generate-bindings web/src/mygo.ts`，`git diff` 只应有预期字段。
- **页面以事件为准**：系统消息不渲染；工具结果只进卡片；消息 ID 不要改回 `role+timestamp`。
- **冒烟隔离**：真实 key 只走环境变量；`HOME`/`USERPROFILE` 指到临时目录；`-smoke` 默认临时 cwd。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...`、`cd web && npm run check && npm run build`、`bash scripts/check-docs.sh`。
- **PowerShell 改文件会毁中文**：用编辑工具（P-006）。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG。
- **导入边界**：正式代码只有 `internal/kernel` 引用 `pi/...`；MyGo 只在 `desktop/`、`cmd/locowrk`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100、代码单文件 ≤500、CONTEXT ≤150、handoff 留 5 份。
- 踩坑索引：`P-001`~`P-008`；方法：`skills/S-001-runtime-comparison-spike.md`。
