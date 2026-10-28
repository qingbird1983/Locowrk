# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**仓库文档基线与治理规范落盘**）

## 当前阶段与状态

- **Phase 0（仓库与文档基线）**：文档已落盘，**待所有者确认**；确认后打 `phase-0.end`。
- **Phase 1（内核接入与事件流）**：文档状态「评审中」，未开工，无代码。
- 看板唯一来源：`docs/roadmap/00-roadmap.md`。
- 仓库尚未 `go mod init`；根目录只有文档、脚本与 CI 占位。

## 上一轮完成了什么

- 读完 `docs/architecture/pi-go-desktop.md`（v1.1 方案）并把结论拆成 `docs/architecture/01~05`：总体分层、内核封装、关键接缝、插件系统、会话与资源。
- 建立治理骨架：`docs/README.md` 文档地图、9 篇规范、7 篇 ADR、4 篇调研、8 个阶段文档、Phase 0 报告、`.agents/` 交接机制、`CHANGELOG.md`。
- 核对两个外部依赖的事实：`sky-valley/pi` 本地快照为 `6f6131ba`（2026-10-25，分支 main，`env/v1.4.28-14-g6f6131b`）；MyGo 为 v0.7.1（MIT，纯 Go 无 cgo，要求 Go 1.27.3+，提供原生 `ui` 与系统 WebView 两条路径、Typed IPC）。
- 记录关键缺口：`sky-valley/pi` **没有 pi 的 extensions runtime**，也尚未移植 prompt templates；A 轨兼容必须由本项目的适配层补齐。
- 建立文档门禁 `scripts/check-docs.sh`（孤儿文档、个人绝对路径、必需入口）与 CI（有 `go.mod` 后自动跑 fmt/vet/test）。

## 下一步（按优先级）

1. **所有者确认**：许可证（暂定 MIT）、项目名、Phase 0 报告与 Phase 1 文档；确认后打 tag `phase-0.end`。
2. **Phase 1 设计定稿**：`docs/roadmap/phase-1-kernel-events.md` 从「评审中」转「定稿」。
3. **Phase 1 实现**：`go mod init` → 依赖 `github.com/sky-valley/pi`（锁定提交）→ `internal/kernel` + `internal/eventbus` → CLI/stdout 事件验证 → JSONL 会话读写验证。
4. Phase 1 收尾时做首次例行规范复查（standards/09）。

## 未决问题

- **主界面渲染路径**：会话流走 MyGo 原生 `ui.View` 还是 WebView + TS？Phase 3 开工前用一次 spike 定稿，必要时新增 ADR。
- **A 轨桥接形态**：直接桥接 pi 的 TS 扩展（需要 Node 或 JS 运行时），还是用 Go 重写选定扩展？Phase 2 设计时定；当前只承诺“兼容编排语义”。
- **`sky-valley/pi` 版本策略**：固定提交还是跟随 release tag？当前固定提交；升级规则见 ADR-0001，首次升级前补 ADR。
- **许可证与名称**：均待所有者确认。
- **MyGo v0.x API 漂移**：需要把 MyGo 用法限制在 `desktop/` 与 `internal/bridge`，内核与适配层不依赖 GUI。

## 关键提醒

- **内核零 GUI**：`internal/kernel`、`internal/adapter`、`internal/eventbus` 不得 import MyGo；测试用纯 Go。
- **不写自己的会话格式**：读写统一走 `coding.SessionRecorder` / `LoadSessionTree`（ADR-0005），避免恢复语义分叉。
- **项目技能默认不加载**：`sky-valley/pi` 是 headless 默认，`<cwd>/.pi/skills` 需要显式 `TrustProject`；桌面端必须用用户可见的授权，不得静默默认信任。
- **事件类型不要自造**：Agent 生命周期事件以 `agent.Ev*` 为准（`agent_start` / `turn_*` / `message_*` / `tool_execution_*`）；只新增编排级事件（subagent / workflow / steering）。
- **Agent 事件不可阻塞**：订阅者慢不得拖住循环；控制事件可靠排队，delta 事件允许合并/丢弃并计数。
- **MyGo 线程模型**：`App.Run` 必须从 main 调用；事件回调在主线程；绑定服务方法在各自 goroutine。插件视图只在主线程构建。
- **`sky-valley/pi` 本地没有 CI**：本仓库的门禁是唯一防线；升级必须跑全量 `go test -race ./...`。
- **文档门禁**：新增文档必须登记 `docs/README.md`，否则 `scripts/check-docs.sh` 失败。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、CONTEXT ≤150 行；收尾核对。
- 踩坑索引：暂无（Phase 1 起积累）。
