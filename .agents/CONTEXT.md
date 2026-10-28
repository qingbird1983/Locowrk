# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 5 设计定稿**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3 / 4**：✅ 完成，tag 已打（`phase-4.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），规则见 `pi/FORK.md`。
- **Phase 5（WebView 混合渲染与 Diff 审阅）**：🟡 设计定稿 + 双 WebView 前置 spike 通过（2026-10-28）；功能实现未开始，tag 未打。
  - `docs/roadmap/phase-5-webview-plugins.md` 定稿；设计 `docs/design/phase-5/webview.md` + `diff.md`。
  - 关键决策 ADR-0014：`session.tab` 是宿主托管的 WebView 标签页，每页一张，关闭即销毁；`DesktopPlugin` 接口不变。
  - 上限：最多 4 个 Diff 标签；diff 2 万行 / 8 MiB / 200 文件；git 5 秒超时、只读、无 shell。
  - 前置 spike 通过：同窗两张 WebView、切换显隐、A 页滚动/输入保持、原生 Popover 覆盖页面、`Destroy` 成功；证据 research/03 §11 与 `design/phase-5/spike-tabs.png`。
- **界面范围（所有者 2026-10-28 决定）**：当前窗口是**功能粗稿**；布局密度、配色、按钮尺寸、图标、高 DPI、输入法观感在全部功能完成后统一做“界面 UI 重设计”，**不要在各阶段打磨界面**。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只保证可构建。

## 上一轮完成了什么

1. Phase 5 设计定稿：标签页宿主、Diff 服务/解析/DTO、Diff 页面与虚拟列表；ADR-0014 记录 `session.tab` 语义。
2. 新建 `fixtures/spike-mygo-tabs/`：headless 标签/弹层几何、隐藏窗口页面与销毁、可见窗口切换与原生捕获三档验证。
3. Spike 全部通过：切换只摆放活动页；A 页 `scrollTop=320` 与输入 `hello` 切回仍保持；B `IsDestroyed=true`；OS 截屏确认弹层压在网页上。
4. 记录测试边界 P-009：`ui.NativeWebView` 含 internal 类型，仓库外无法实现假接口；改用零值 `*mygo.WebView` 句柄 + 注入工厂。
5. 同步 research/03 §11、Phase 5 设计/路线图、CHANGELOG 0.9.4、CONTEXT 与 H-016。

## 下一步（按优先级）

1. **Phase 5 实现**：`desktop/tabs.go`（TabHost/标签栏/工厂）→ `diff*.go`（git/解析/DTO/绑定）→ `web/diff.html` + `web/src/diff/*` → 工具卡片接线 → `diff` 冒烟 + 7 场景回归。
2. **可见窗口人工项**：两页焦点往返、中文输入法、高 DPI、标签栏尺寸观感（后者留 UI 重设计）。
3. **Phase 6 预备**：真实 Pi 编排插件接入、三分类矩阵、steering 回传；与插件数据源一起评估（未开始）。
4. **UI 重设计**：等功能完成后单独立项；当前只改功能，不改视觉。
5. **发布工程**：仍暂缓；`standards/08 §6`“禁止提交 dist/”与前端 embed 入库的冲突留到该阶段一并修订。

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
