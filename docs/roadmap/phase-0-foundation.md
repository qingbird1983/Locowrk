# Phase 0：仓库与文档基线

- 状态：定稿
- 日期：2026-10-28
- 关联：ADR-0006；docs/README.md；design/phase-0/report.md

## 1. 目标

在不写实现代码的前提下，把项目的“事实、决策、规则、路线”落盘，使任何新会话只读文档就能接手；并初始化 Git 仓库。

## 2. 开工前置条件

无（本阶段就是基线）。

## 3. 做什么

- [x] 根入口：`README.md` / `README.zh-CN.md` / `AGENTS.md` / `CHANGELOG.md` / `LICENSE` / `.gitignore` / `.editorconfig`
- [x] 文档地图与术语表：`docs/README.md`、`docs/glossary.md`
- [x] 架构：立项方案 + `01-overview` ~ `05-session-and-resources`
- [x] 决策：ADR-0001 ~ 0007（内核、宿主、兼容边界、进程隔离、会话、治理、许可证）
- [x] 调研：`sky-valley/pi`、pi 扩展面、MyGo、治理参考
- [x] 路线图：总览 + 八个阶段文档（Phase 0 ~ 7）
- [x] 规范：9 篇（结构/Git/文档/代码/测试/安全/AI/体量/复查）
- [x] AI 工作记忆：`.agents/README.md`、`.agents/CONTEXT.md`
- [x] 文档门禁：`scripts/check-docs.sh`；CI 占位（有 `go.mod` 后自动跑 Go 门禁）
- [x] 初始化 Git 仓库，首个提交

## 4. 不做什么

- 不写任何 Go/TS 实现代码，不 `go mod init`，不引入依赖。
- 不实现插件协议、不建 UI、不打包。
- 不复制参考项目（PI-Desktop、pi-flash、Timo-pi）的代码或文档文字。

## 5. 验收场景（所有者可逐条核对）

1. **能读懂**：README 说清“这是什么、给谁用、现在在哪一步”；术语表能解释文档里的行话。
2. **能接手**：新会话按 `AGENTS.md` → `docs/README.md` → `.agents/CONTEXT.md` 的顺序读完，知道下一步做什么。
3. **能决策**：7 个 ADR 覆盖了关键取舍，每个都有备选与否决理由。
4. **能验收**：每个阶段都有“做/不做/验收场景”，不需要临场发挥。
5. **能守住**：9 篇规范覆盖文档、代码、测试、安全、体量与规范复查。
6. **无孤儿**：`scripts/check-docs.sh` 通过（所有文档已登记、无个人绝对路径、必需入口齐全）。
7. **无代码**：仓库中没有 `.go` 实现文件；CI 的 Go 任务自动跳过。
8. **可追溯**：CHANGELOG 有一条面向非技术读者的初始化记录；Git 首个提交存在。

## 6. 收尾物

- `docs/design/phase-0/report.md`（逐条验收结果、体量核对、已知问题）
- 所有者确认后打 tag `phase-0.end`。

## 7. 风险

- **一次性文档量大**：可能有过时或互相矛盾。对策：Phase 1 收尾做首次规范复查；发现矛盾当轮修。
- **上游事实可能变化**：`sky-valley/pi` 与 MyGo 都在快速演进。对策：research 标注日期与版本，升级时同步。
- **主壳路径未定**：Phase 3 前用 spike 定稿，避免返工。
