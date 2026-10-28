# 工作上下文交接卡

- 更新：2026-10-28（会话主题：**本地目录改名为 Locowrk，新路径开工检查**）

## 当前阶段与状态

- **项目名：Locowrk**（拼写是 `wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀）。远端：`github.com/qingbird1983/Locowrk`（**私有**）；旧 Rust 历史在 `archive/pre-go-locowrk`。
- **本地目录已改名完成**：仓库根目录即 `E:\Warehouse\AIwork\Locowrk`（旧名 `pi-go-desktop` 已不存在；旧路径只在 git 历史与 ADR-0007/CHANGELOG 的历史记录里）。
- **Go module**：`github.com/qingbird1983/locowrk`（module 小写，仓库显示名保留 `Locowrk`）。
- **Phase 0（仓库与文档基线）**：✅ **完成**，tag `phase-0.end`（2026-10-28 所有者确认 MIT 与报告）。
- **Phase 1（内核接入与事件流）**：文档已**定稿**，本轮开工。目标：内核 + 事件流 + JSONL 会话恢复，不做界面。
- **Phase 2（适配器与 A 轨运行时选型）**：文档新增限时 spike（ADR-0008），待 Phase 1 后开工。
- 看板唯一来源：`docs/roadmap/00-roadmap.md`。

## 上一轮完成了什么

1. 所有者执行目录改名：`pi-go-desktop` → `Locowrk`；本会话在新路径接续，未重新 `git init`、未动 remote。
2. 开工检查全部通过：路径、remote、工作区干净、`check-docs: OK`。
3. 清掉“目录待迁移”的过期表述（CONTEXT、Phase 0 报告、看板、Phase 1 备注）。
4. Phase 0 收尾：所有者确认 MIT，看板置完成，打 tag `phase-0.end`，Phase 1 文档转定稿。
5. 沉淀：`pitfalls/P-001`（状态类事实不写进不可改的 ADR）、`handoff/H-001`（迁移轮交接）。

## 下一步（按优先级）

1. **Phase 1 实现**（feature 分支 `phase-1/kernel-events`）：`go mod init` → 依赖 `github.com/sky-valley/pi`（锁定提交）→ `internal/eventbus` + `internal/kernel` → `cmd/locowrk-smoke` 事件验证 → JSONL 会话读写验证。
2. **验收**：逐条跑 `docs/roadmap/phase-1-kernel-events.md` §5 的 7 个场景，结果写 `docs/design/phase-1/report.md`。
3. **首次规范复查**：Phase 1 收尾按 standards/09 做。
4. **Phase 2 设计定稿**：把 runtime spike 的样本、指标、淘汰线写细；spike 后补 ADR-0009。

## 未决问题

- **A 轨运行时**：Phase 2 spike 后在 ADR-0009 定；此前不得对外说“支持 pi 扩展”。
- **主界面渲染路径**：会话流走 MyGo 原生 `ui.View` 还是 WebView + TS？Phase 3 开工前用一次 spike 定稿，必要时新增 ADR。
- **`sky-valley/pi` 版本策略**：固定提交还是跟随 release tag？当前固定提交；升级规则见 ADR-0001。
- **许可证**：MIT（ADR-0007，所有者已于 2026-10-28 确认）；正式发布前再做一次名称/商标可用性检查，不另写 ADR。
- **MyGo v0.x API 漂移**：MyGo 用法限制在 `desktop/` 与 `internal/bridge`，内核与适配层不依赖 GUI。

## 关键提醒

- **项目名是 `Locowrk`**：`wrk`，不是 `work`/`wrok`；不加 `-pi` 后缀。远端是私有仓库，未经所有者允许不得转公开或外发。
- **ADR 不可改**：`docs/adr/` 只增不改；ADR-0007 里“目录仍是 `pi-go-desktop`”是当时的记录，不是现状。状态类事实写 CONTEXT/看板/CHANGELOG（见 `pitfalls/P-001`）。
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
- 踩坑索引：`pitfalls/P-001-status-facts-in-adr.md`。
