# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**插件兼容策略对齐 + A 轨运行时选型提前**）

## 当前阶段与状态

- **Phase 0（仓库与文档基线）**：文档已落盘，**待所有者确认**；确认后打 `phase-0.end`。
- **Phase 1（内核接入与事件流）**：文档状态「评审中」，未开工，无代码。
- **Phase 2（适配器与 A 轨运行时选型）**：文档新增限时 spike（ADR-0008），待 Phase 1 后开工。
- 看板唯一来源：`docs/roadmap/00-roadmap.md`。
- 仓库尚未 `go mod init`；根目录只有文档、脚本与 CI 占位。

## 上一轮完成了什么

- 建立治理基线：文档地图、9 篇规范、8 篇 ADR、4 篇调研、8 个阶段文档、Phase 0 报告、`.agents/` 交接机制、CHANGELOG、文档门禁与 CI 占位。
- 核对依赖事实：`sky-valley/pi` 快照 `6f6131ba`（2026-10-25，`env/v1.4.28-14-g6f6131b`）；MyGo v0.7.1（MIT、纯 Go、Go 1.27.3+、原生 UI + WebView）。
- 记录关键缺口：`sky-valley/pi` 没有 extensions runtime 与 prompt templates；A 轨必须自建。
- **对齐 Timo-pi 的插件兼容策略**：ADR-0003 从“UI 调用抛错”改为“三分类矩阵 + no-op + 诊断”；基础对话框映射宿主；安全拒绝与兼容降级分开记录。
- **新增 ADR-0008**：A 轨运行时（QuickJS / Node 子进程 / Go 重写）在 Phase 2 用 5 天 spike 定稿，产出 ADR-0009；Phase 6 只接入，不再选型。
- 同步更新：architecture/03 #6、architecture/04、research/02、phase-2、phase-6、roadmap、docs/README、两份 README、Phase 0 报告、CHANGELOG。

## 下一步（按优先级）

1. **所有者确认**：许可证（暂定 MIT）、项目名、Go 模块路径、Phase 0 报告与 Phase 1 文档；确认后打 tag `phase-0.end`。
2. **Phase 1 设计定稿**：`docs/roadmap/phase-1-kernel-events.md` 从「评审中」转「定稿」。
3. **Phase 1 实现**：`go mod init` → 依赖 `github.com/sky-valley/pi`（锁定提交）→ `internal/kernel` + `internal/eventbus` → CLI/stdout 事件验证 → JSONL 会话读写验证。
4. **Phase 2 设计定稿**：把 runtime spike 的样本、指标、淘汰线写细；spike 后补 ADR-0009。
5. Phase 1 收尾时做首次例行规范复查（standards/09）。

## 未决问题

- **A 轨运行时**：Phase 2 spike 后在 ADR-0009 定；此前不得对外说“支持 pi 扩展”。
- **主界面渲染路径**：会话流走 MyGo 原生 `ui.View` 还是 WebView + TS？Phase 3 开工前用一次 spike 定稿，必要时新增 ADR。
- **`sky-valley/pi` 版本策略**：固定提交还是跟随 release tag？当前固定提交；升级规则见 ADR-0001。
- **许可证、名称、模块路径**：均待所有者确认。
- **MyGo v0.x API 漂移**：MyGo 用法限制在 `desktop/` 与 `internal/bridge`，内核与适配层不依赖 GUI。

## 关键提醒

- **内核零 GUI**：`internal/kernel`、`internal/adapter`、`internal/eventbus` 不得 import MyGo；测试用纯 Go。
- **不写自己的会话格式**：读写统一走 `coding.SessionRecorder` / `LoadSessionTree`（ADR-0005）。
- **项目技能默认不加载**：`<cwd>/.pi/skills` 需要显式 `TrustProject`；桌面端必须用户可见授权。
- **插件兼容**：支持项真做、缓做项 `not_implemented`、TUI 项 no-op + `unsupported`；**永不 throw**；每插件每成员诊断一次。
- **安全拒绝单独记录**：文件写删/执行/网络未声明仍 fail-closed，不能混进“兼容性诊断”。
- **事件类型不要自造**：生命周期以 `agent.Ev*` 为准；只新增编排级事件（subagent / workflow / steering）。
- **Agent 事件不可阻塞**：控制事件可靠排队，delta 允许合并/丢弃并计数。
- **MyGo 线程模型**：`App.Run` 从 main 调用；监听器在主线程；绑定服务方法在各自 goroutine；插件视图只在主线程构建。
- **本地没有 CI**：`sky-valley/pi` 自身无 CI，本仓库门禁是唯一防线；升级跑全量 `go test -race ./...`。
- **文档门禁**：新增文档必须登记 `docs/README.md`，否则 `scripts/check-docs.sh` 失败。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、CONTEXT ≤150 行；收尾核对。
- 踩坑索引：暂无（Phase 1 起积累）。
