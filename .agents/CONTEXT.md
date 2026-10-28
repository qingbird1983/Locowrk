# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 4 收尾与 UI 范围决定**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3 / 4**：✅ 完成，tag 已打（`phase-4.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），规则见 `pi/FORK.md`。
- **Phase 4（PluginHost 与原生插件）**：✅ 功能与数据流验收 8/8（[报告](../docs/design/phase-4/report.md)）。
  - 主窗口“原生外框 + 内嵌 WebView”（ADR-0013）；Phase 3 页面原样。
  - 三个插件：编排树、workflow 时间线、工具检查器；工具卡片点击联动；启停与右栏状态持久化；单插件 panic 自隔离。
  - 读数：100 节点 + 500 调用 0.70 ms/帧；`real` 769 ms 返回 `pong`；六个 Phase 3 场景回归通过。
- **界面范围（所有者 2026-10-28 决定）**：当前窗口是**功能粗稿**；布局密度、配色、按钮尺寸、图标、高 DPI、输入法观感在全部功能完成后统一做“界面 UI 重设计”，**不要在各阶段打磨界面**。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只保证可构建。
- **看板唯一来源**：`docs/roadmap/00-roadmap.md`；流水唯一来源：`CHANGELOG.md`。

## 上一轮完成了什么

1. 补跑真实 provider 冒烟（769 ms `pong`）；Phase 3 六场景在原生外框下全部通过。
2. 数据流边界测试：树 200 节点上限、workflow 终态不回退、工具 200 条环形缓冲、`clipText` UTF-8。
3. 报告转定稿；看板 Phase 4 完成；backlog 增“界面 UI 重设计”；CHANGELOG 0.9.2；tag `phase-4.end`。
4. handoff 滚动到 H-010…H-014；pitfalls 增 P-007（窗口类型）、P-008（隐藏窗口/rAF）。

## 下一步（按优先级）

1. **Phase 5 设计稿**：WebView 混合渲染 + Diff 审阅插件；首次启用 `session.tab`。先写 `docs/design/phase-5/`，定稿后再实现。
2. **Phase 6 预备**：真实 Pi 编排插件接入、三分类矩阵、steering 回传；与插件数据源（子代理/workflow 历史回放）一起评估。
3. **UI 重设计**：等功能完成后单独立项；当前只改功能，不改视觉。
4. **发布工程**：仍暂缓；`standards/08 §6`“禁止提交 dist/”与前端 embed 入库的冲突留到该阶段一并修订。

## 未决问题

- **插件状态**：会话切换即清空，切回旧会话不恢复树/时间线；Phase 6 接真实数据源时评估回放。
- **隐藏验收边界**：Windows 隐藏窗口不构建原生内容，页面 rAF 也暂停；数据用隐藏冒烟，渲染用 `ui.NewTester`，crash 用 `-show`（P-008）。
- **panic 恢复**：丢一帧、当帧构建计数偏移；测试未出现输入丢失，暂不做双帧预检。
- **长会话内存**：5k 约数 MB，10k+ 未测；`pi/` 约 2.6 MB / 132 文件（ADR-0012 豁免）。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。
- **Windows 之外**：MyGo/WebView 只在 Windows 冒烟。

## 关键提醒

- **窗口类型**（P-007）：原生窗口没有 `win.Page()`；用 `web.Page()` + `Event.EmitPage`。
- **隐藏窗口**（P-008）：不画原生内容、页面 rAF 暂停；不要用隐藏冒烟断言渲染。
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
