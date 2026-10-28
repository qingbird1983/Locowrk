# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**项目更名 Locowrk、插件兼容对齐、A 轨运行时选型**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；本地目录目前仍叫 `pi-go-desktop`。
- **Go module**：`github.com/qingbird1983/locowrk`（module 小写，仓库显示名保留 `Locowrk`）。
- **Phase 0（仓库与文档基线）**：文档已落盘，**待所有者确认**；确认后打 `phase-0.end`。
- **Phase 1（内核接入与事件流）**：文档状态「评审中」，未开工，无代码。
- **Phase 2（适配器与 A 轨运行时选型）**：文档新增限时 spike（ADR-0008），待 Phase 1 后开工。
- 看板唯一来源：`docs/roadmap/00-roadmap.md`。
- 仓库尚未 `go mod init`；根目录只有文档、脚本与 CI 占位。

## 上一轮完成了什么

1. 建立治理基线：文档地图、9 篇规范、8 篇 ADR、5 篇调研、8 个阶段文档、Phase 0 报告、`.agents/` 交接机制、CHANGELOG、文档门禁与 CI 占位。
2. 核对依赖事实与缺口：`sky-valley/pi` 快照 `6f6131ba`（2026-10-25，`env/v1.4.28-14-g6f6131b`）；MyGo v0.7.1（MIT、纯 Go、Go 1.27.3+、原生 UI + WebView）；上游没有 extensions runtime 与 prompt templates。
3. **对齐 Timo-pi**：ADR-0003 改为“三分类 + no-op + 诊断”；新增 ADR-0008，把 A 轨运行时选型提前到 Phase 2 的 5 天 spike。
4. **研究 PI-Desktop（LGPL，只读）**：trusted extension 子集、权限+范围、进程监督与“隔离≠沙箱”的教训落盘 `research/05`；据此修订 ADR-0004/0008 与插件架构。
5. 同步更新：architecture/03 #6、architecture/04、research/02、phase-2、phase-6、roadmap、docs/README、两份 README、Phase 0 报告、CHANGELOG。

## 下一步（按优先级）

1. **所有者确认**：许可证、仓库本地目录是否迁移、Phase 0 报告与 Phase 1 文档；确认后打 tag `phase-0.end`。
2. **命名迁移（可选）**：是否把仓库目录、Go module、远端仓库从 `pi-go-desktop` 改为 `locowrk`；改名会改变本会话工作目录，需单独执行。
3. **Phase 1 设计定稿**：`docs/roadmap/phase-1-kernel-events.md` 从「评审中」转「定稿」。
4. **Phase 1 实现**：`go mod init` → 依赖 `github.com/sky-valley/pi`（锁定提交）→ `internal/kernel` + `internal/eventbus` → CLI/stdout 事件验证 → JSONL 会话读写验证。
5. **Phase 2 设计定稿**：把 runtime spike 的样本、指标、淘汰线写细；spike 后补 ADR-0009。
6. Phase 1 收尾时做首次例行规范复查（standards/09）。

## 未决问题

- **A 轨运行时**：Phase 2 spike 后在 ADR-0009 定；此前不得对外说“支持 pi 扩展”。
- **主界面渲染路径**：会话流走 MyGo 原生 `ui.View` 还是 WebView + TS？Phase 3 开工前用一次 spike 定稿，必要时新增 ADR。
- **`sky-valley/pi` 版本策略**：固定提交还是跟随 release tag？当前固定提交；升级规则见 ADR-0001。
- **许可证、仓库本地目录**：许可证待确认；目录是否从 `pi-go-desktop` 改为 `locowrk` 见“下一步”第 2 条。远端与 module 已定。
- **MyGo v0.x API 漂移**：MyGo 用法限制在 `desktop/` 与 `internal/bridge`，内核与适配层不依赖 GUI。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀。远端是私有仓库，未经所有者允许不得转公开或外发。目录/模块迁移完成前，历史名 `pi-go-desktop` 只出现在旧路径与 git 历史里。
- **内核零 GUI**：`internal/kernel`、`internal/adapter`、`internal/eventbus` 不得 import MyGo；测试用纯 Go。
- **不写自己的会话格式**：读写统一走 `coding.SessionRecorder` / `LoadSessionTree`（ADR-0005）。
- **项目技能默认不加载**：`<cwd>/.pi/skills` 需要显式 `TrustProject`；桌面端必须用户可见授权。
- **插件兼容**：支持项真做、缓做项 `not_implemented`、TUI 项 no-op + `unsupported`；**永不 throw**；每插件每成员诊断一次。
- **A 轨是受信任代码**：能注册工具/钩子就等同 Agent 权限；进程隔离只防崩溃，不是沙箱（research/05）。
- **权限 + 范围**：manifest 的 `fs.*` 必须带路径范围、`net` 必须带域名白名单；缺失 = 拒绝；全树写删非法。
- **安全拒绝单独记录**：文件写删/执行/网络未声明仍 fail-closed，不能混进“兼容性诊断”。
- **事件类型不要自造**：生命周期以 `agent.Ev*` 为准；只新增编排级事件（subagent / workflow / steering）。
- **Agent 事件不可阻塞**：控制事件可靠排队，delta 允许合并/丢弃并计数。
- **MyGo 线程模型**：`App.Run` 从 main 调用；监听器在主线程；绑定服务方法在各自 goroutine；插件视图只在主线程构建。
- **本地没有 CI**：`sky-valley/pi` 自身无 CI，本仓库门禁是唯一防线；升级跑全量 `go test -race ./...`。
- **文档门禁**：新增文档必须登记 `docs/README.md`，否则 `scripts/check-docs.sh` 失败。
- **体量**：docs 单篇 ≤300 行、ADR ≤100 行、CONTEXT ≤150 行；收尾核对。
- 踩坑索引：暂无（Phase 1 起积累）。
