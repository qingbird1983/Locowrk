# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 5 设计定稿**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3 / 4**：✅ 完成，tag 已打（`phase-4.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），规则见 `pi/FORK.md`。
- **Phase 5（WebView 混合渲染与 Diff 审阅）**：🟡 功能实现完成（2026-10-28），待人工验收与报告定稿；tag 未打。
  - 设计 `docs/design/phase-5/webview.md` + `diff.md`；ADR-0014（`session.tab` 宿主托管）。
  - 标签页：`desktop/tabs.go`；Diff：`desktop/diff.go` + `diff_parse.go`；页面：`web/diff.html` + `web/src/diff/*`。
  - 冒烟：`diff`（直接打开）与 `diff-tool`（工具卡片链路）；10k 行 `diffMs≈425`；切标签滚动/模式保持；关闭后 `tabs=0`。
  - 待人工：真实窗口焦点回网页、中文输入法、高 DPI；macOS/Linux 仅构建。
- **界面范围（所有者 2026-10-28 决定）**：当前窗口是**功能粗稿**；布局密度、配色、按钮尺寸、图标、高 DPI、输入法观感在全部功能完成后统一做“界面 UI 重设计”，**不要在各阶段打磨界面**。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只保证可构建。

## 上一轮完成了什么

1. 实现 `TabHost`：会话 + 最多 4 个 Diff 标签，宿主创建/摆放/销毁 WebView；`⋯` 菜单、刷新事件、会话切换清理；无窗口测试用零值句柄 + 工厂（P-009）。
2. 实现 Diff：固定只读 git 命令、15k 行解析与 DTO、敏感路径/截断/行号；`Desktop.OpenDiff` 从工具卡片打开并按工具路径定位。
3. 实现 Diff 页面：并排/统一、虚拟列表、折叠、轻量着色、复制路径、刷新、截断/错误提示；`diff-reload` 保持模式与滚动。
4. 冒烟 `diff`/`diff-tool` 通过；10k 行约 425 ms；切标签滚动与模式保持；关闭后 `tabs=0`；Phase 3/4 场景回归通过（`restore` 未种子）。
5. 文档：Phase 5 报告草案、CHANGELOG 0.9.5、看板/路线图状态、CONTEXT/H-017。

## 下一步（按优先级）

1. **人工验收 Phase 5**：可见窗口里切标签后键盘焦点、菜单关闭后焦点回网页、中文输入法、高 DPI；确认后把报告转定稿并打 `phase-5.end`。
2. **收尾**：补 `real`/`restore` 冒烟（有 key / 种子会话），更新报告与看板，tag。
3. **Phase 6 预备**：真实 Pi 编排插件接入、三分类矩阵、steering 回传。
4. **UI 重设计**：功能完成后单独立项；当前标签栏与 Diff 页仍是功能粗稿。
5. **发布工程**：仍暂缓；`standards/08 §6` 与前端 embed 的冲突留到该阶段。

## 未决问题

- **双 WebView 未实测**：焦点、IME、高 DPI、销毁后进程回收；spike 是硬前置。
- **标签页内存**：4 个上限；若单页常驻过高，改为销毁非活动标签（会丢状态，需重评验收 3）。
- **Diff 文本选择**：虚拟列表只能选已挂载行，跨屏复制不承诺。
- **插件状态**：会话切换即清空，切回旧会话不恢复树/时间线；Phase 6 接真实数据源时评估回放。
- **隐藏验收边界**：Windows 隐藏窗口不构建原生内容、页面 rAF 暂停；数据用隐藏冒烟，渲染用 `ui.NewTester`，crash 用 `-show`（P-008）。
- **panic 恢复**：丢一帧、当帧构建计数偏移；测试未出现输入丢失，暂不做双帧预检。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。

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
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG。
- **导入边界**：正式代码只有 `internal/kernel` 引用 `pi/...`；MyGo 只在 `desktop/`、`cmd/locowrk`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100、代码单文件 ≤500、CONTEXT ≤150、handoff 留 5 份。
- 踩坑索引：`P-001`~`P-009`；方法：`skills/S-001-runtime-comparison-spike.md`。
