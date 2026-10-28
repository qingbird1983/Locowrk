# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**Phase 3 验收通过，主壳性能与正确性收口**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端 `github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录**：`E:\Warehouse\AIwork\Locowrk`；Go module `github.com/qingbird1983/locowrk`。
- **Phase 0 / 1 / 2 / 3**：✅ 完成，tag 已打（`phase-3.end`）。
- **内核形态**：✅ 仓库内 fork（ADR-0012），源码 `pi/`，基线 `sky-valley/pi@6f6131ba`（MIT），维护规则见 `pi/FORK.md`。
- **Phase 3（MyGo 最小宿主）**：✅ 验收 8/8 通过，窗口与中文输入已人工确认（[报告](docs/design/phase-3/report.md)）：
  - ✅ 主壳 `cmd/locowrk` + `desktop/` + `web/`；真实 key、工具卡片、取消、重启恢复、空闲 CPU、5k 消息、边界与安全均有脚本化证据。
  - ✅ 消息 ID 唯一化（角色+时间戳+transcript 位置），跨轮/重启稳定。
  - ✅ 窗口化消息列表 + 按帧合批；5,000 条真实会话 208 ms 打开，挂载 18 行。
- **Phase 4**：未开始（开工前先写设计文档）。
- **发布工程（暂缓，backlog）**：钥匙串、Windows 打包/签名（需证书）；macOS/Linux 按 ADR-0007 只需保证可构建，已完成交叉编译验证，运行验证待真机。
- **看板唯一来源**：`docs/roadmap/00-roadmap.md`；流水唯一来源：`CHANGELOG.md`。

## 上一轮完成了什么

1. `internal/kernel`：DTO ID 加 transcript 位置；`idSource` 记录实时事件位置，`History` 同序重放；跨轮唯一性测试。
2. `web/src/transcript.ts`：窗口化列表（上下 spacer + 实测行高）、rAF 合批、工具结果并入卡片、跳过系统消息、用户消息只由事件回显。
3. `web/src/smoke.ts` + `-smoke-scenario`（faux/real/tool/cancel/long/restore）、`-smoke-expect`、`-hidden`；`scripts/seed-long-session.mjs` 造 5,000 条真实会话。
4. `Info.credentials` 只返回布尔值；状态行显示“凭证：已配置/未配置”；生成客户端已重跑。
5. 文档：`design/phase-3/report.md`；shell/frontend 设计稿、看板、CHANGELOG 0.8.0、docs 地图同步。
6. Phase 3 收尾：所有者确认窗口与中文输入；安装包/签名、钥匙串按所有者决定暂缓（不进主线）；macOS/Linux 按 ADR-0007 以交叉编译复核（4 个目标通过）；打 tag `phase-3.end`。

## 下一步（按优先级）

1. **Phase 4 设计稿**：PluginHost + 首批原生 `ui.View`；插件视图复用 `transcript` 的窗口化模型。
2. **发布工程（暂缓）**：所有者明确暂不发布；打包/签名/钥匙串留在 backlog，macOS/Linux 运行验证待需要时补真机。
3. **性能复核**：10k+ 消息内存；启用压缩/回放前复核位置型消息 ID。

## 未决问题

- **消息 ID 边界**：基于“追加顺序”；若压缩/回放插入未发 `message_start` 的消息，实时与历史可能错位。Phase 3 未启用压缩。
- **长会话内存**：全量行数据留在内存；5k 约数 MB，10k+ 未测。
- **`pi/` 体量**：约 2.6 MB / 132 文件，ADR-0012 已登记豁免；本地改动要防止堆业务逻辑。
- **能力闸边界**：Node 原生 fs/net/exec 绕过 shim，按受信任插件处理。
- **Windows 之外**：MyGo/WebView 只在 Windows 冒烟。

## 关键提醒

- **页面以事件为准**：不要恢复“本地先插用户消息”的做法；系统消息不渲染；工具结果只进卡片。
- **消息 ID**：不要改回 `role+timestamp`；改 `internal/kernel/translate.go` 的 `idSource` 前先想清楚重启重放。
- **前端构建顺序**：改 `web/src` 后先 `npm run build`，再 `go build`（`dist/` 被 embed 且入库）；契约变更后 `go run ./cmd/locowrk -generate-bindings web/src/mygo.ts` 并确认 `git diff` 只有预期字段。
- **冒烟隔离**：真实 key 只走环境变量；运行前把 `HOME`/`USERPROFILE` 指到临时目录；`-smoke` 默认临时 cwd，不写仓库。
- **验收命令**：见 report §5；长会话先 `node scripts/seed-long-session.mjs <home> <cwd> 5000`。
- **PowerShell 改文件会毁中文**：用编辑工具；读控制台乱码不代表文件坏（P-006）。
- **提交前门禁**：`gofmt -l . && go vet ./... && go test -race ./...`、`cd web && npm run check && npm run build`、`bash scripts/check-docs.sh`。
- **ADR 不可改**：`docs/adr/` 只增不改；状态类事实写 CONTEXT/看板/CHANGELOG。
- **导入边界**：正式代码只有 `internal/kernel` 引用 `pi/...`；实验/测试可为 fixture 直接引用；MyGo 只在 `desktop/`、`cmd/locowrk`。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、代码单文件 ≤500 行、CONTEXT ≤150 行。
- 踩坑索引：`P-001`~`P-006`；方法：`skills/S-001-runtime-comparison-spike.md`。
