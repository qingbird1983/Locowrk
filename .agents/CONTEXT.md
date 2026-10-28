# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 4 设计定稿与混合渲染 spike**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3**：✅ 完成，tag 已打（`phase-3.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），维护规则见 `pi/FORK.md`。
- **Phase 3（MyGo 最小宿主）**：✅ 验收 8/8 通过；主壳 `cmd/locowrk` + `desktop/` + `web/`。报告见 `docs/design/phase-3/report.md`。
- **Phase 4（PluginHost 与原生插件）**：🟡 **设计定稿，实现未开始**。
  - ADR-0013：主窗口改为“原生外框 + 内嵌 WebView”；Phase 3 前端原样进页面区。
  - 设计稿：`docs/design/phase-4/host.md`（插槽、三个插件、启停/设置、panic 隔离、测试与回归）。
  - 能力已由 `fixtures/spike-mygo-ui/` 验证（读数见 `docs/research/03-mygo-stack.md` §9）。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只保证可构建，交叉编译已验证，运行验证待真机。
- **看板唯一来源**：`docs/roadmap/00-roadmap.md`；流水唯一来源：`CHANGELOG.md`。

## 上一轮完成了什么

1. 读 MyGo v0.7.1 源码定清窗口模型：页面窗口不能建 WebView，原生窗口用 `ui.WebView` 放页面；`Event.Emit(窗口)` 会送到其所有 WebView；`ui.NewTester` 可无窗口多帧。
2. `fixtures/spike-mygo-ui/`（隐藏窗口）：原生栏 + 内嵌页面；页面→Go 679.5 ms、Go→页面事件 684.4 ms、原生窗口 74.6 ms、WebView 93.3 ms；无窗口 100 行 ×120 帧平均 0.18 ms、最慢 0.65 ms。
3. ADR-0013 + `design/phase-4/host.md`：窗口形态、插槽布局、`DesktopPlugin`（构建式 `Build`）、三个插件模型、设置文件、panic 隔离、前端只加 `selectToolCall`。
4. 文档同步：phase-4 转定稿、research/03 §9、docs/README、看板状态、CHANGELOG 0.9.0、pitfall P-007、handoff H-012。

## 下一步（按优先级）

1. **原生外框迁移**：`desktop/shell.go` + `cmd/locowrk` 装配；`win.Page()` → `web.Page()`；先让 Phase 3 六场景（`faux|real|tool|cancel|long|restore`）全绿，再写插件。
2. **Host**：`desktop/plugin.go`、`host.go`（注册、按 Kind 分发、会话切换重置、启停、设置文件、诊断、插槽级 recover）。
3. **三个插件**：`plugins_tree.go`、`plugins_timeline.go`、`plugins_tools.go`；前端工具卡片点击 → `Desktop.selectToolCall`。
4. **验收**：无窗口 `ui.NewTester` 测试；冒烟 `plugin`、`plugin-crash`；六场景回归。
5. **收尾**：插槽契约回填 architecture/04（注明 `session.tool.detail` 修订）、Phase 4 报告、tag `phase-4.end`。

## 未决问题

- **panic 恢复代价**：插槽级 recover 会丢一帧、当帧 `building` 计数偏移（每帧 `context.reset` 会重置）；若实测丢输入，改“探针构建”。
- **页面变窄**：右栏 260 px，窗口最小 900；前端需在 ~620 px 下不破版，焦点与中文输入法要重新人工确认。
- **预留插槽**：`session.tab`、`settings.section` Phase 4 不启用；Phase 5 的 Diff 会先用 `session.tab`。
- **长会话内存**：全量行数据留在内存；5k 约数 MB，10k+ 未测。
- **`pi/` 体量**：约 2.6 MB / 132 文件，ADR-0012 已登记豁免；本地改动要防止堆业务逻辑。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。
- **Windows 之外**：MyGo/WebView 只在 Windows 冒烟。

## 关键提醒

- **窗口类型**（P-007）：原生窗口没有 `win.Page()`；页面求值用 `web.Page()`。不要混用两种窗口 API。
- **页面以事件为准**：不要恢复“本地先插用户消息”；系统消息不渲染；工具结果只进卡片。
- **消息 ID**：不要改回 `role+timestamp`；改 `internal/kernel/translate.go` 的 `idSource` 前先想清楚重启重放。
- **前端构建顺序**：改 `web/src` 后先 `npm run build`，再 `go build`（`dist/` 被 embed 且入库）；契约变更后 `go run ./cmd/locowrk -generate-bindings web/src/mygo.ts` 并确认 `git diff` 只有预期字段。
- **插件纪律**：状态只存数据，`Build` 每帧重建，`ui.Element` 不落状态，不 import `internal/kernel`；事件回调只改数据 + `Invalidate()`。
- **设置文件**：`os.UserConfigDir()/Locowrk/settings.json`；`-config` 可覆盖，冒烟/测试必须指到临时目录。
- **冒烟隔离**：真实 key 只走环境变量；运行前把 `HOME`/`USERPROFILE` 指到临时目录；`-smoke` 默认临时 cwd，不写仓库。
- **验收命令**：见 phase-3 report §5；长会话先 `node scripts/seed-long-session.mjs <home> <cwd> 5000`。
- **PowerShell 改文件会毁中文**：用编辑工具（P-006）。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...`、`cd web && npm run check && npm run build`、`bash scripts/check-docs.sh`。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG。
- **导入边界**：正式代码只有 `internal/kernel` 引用 `pi/...`；MyGo 只在 `desktop/`、`cmd/locowrk`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`P-001`~`P-007`；方法：`skills/S-001-runtime-comparison-spike.md`。
