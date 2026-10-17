# 历史归档（只读）

> 这一层放**过程与记忆**：批次流水、旧路线表、旧变更日志、已退役的设计与已批准的计划。
> 规矩只有三条：**不再往里追加**、**不改写内容**、**要查当时怎么做的就翻这里或 git 记录**。
> 现在有效的东西一律不在这里 —— 规范看 [`../design/`](../design/)，进度看 [`../design/roadmap/`](../design/roadmap/)，
> 每阶段的详细计划看 [`../../specs/`](../../specs/)，决策理由看 [`../adr/`](../adr/)，全仓地图看 [`../../STRUCTURE.md`](../../STRUCTURE.md)。

## 这里有什么

| 文件 | 它是什么 | 原来在哪 |
| --- | --- | --- |
| `ROADMAP-legacy.md` | 旧路线表全文（925 行，含已完成阶段①–㉗、S1/S2、收尾 backlog、插件清单完成度、功能层排期），截至 v2.208 | `docs/design/ROADMAP.md` |
| `ROADMAP-batches.md` | 旧"批次叙述"归档（每批一整段，v2.120–v2.199） | `docs/design/archive/ROADMAP-batches.md` |
| `CHANGELOG-legacy.md` | 旧逐批落点日志的在役窗口（v2.201–v2.208，每版一行最长 13 KB） | `docs/design/CHANGELOG.md` |
| `CHANGELOG-batches.md` | 旧逐批落点日志归档（v2.27–v2.200，139 批） | `docs/design/archive/CHANGELOG-batches.md` |
| `AGENTS-progress.md` | 旧"当前进度"流水（v2.55–v2.206 逐批一条，从 AGENTS.md 挤出来的那些） | `docs/design/archive/AGENTS-progress-archive.md` |
| `INDEX-legacy.md` | 旧文档索引（六类状态表），职责已由根上的 `STRUCTURE.md` 接管 | `docs/design/INDEX.md` |
| `post-audit-roadmap-v1.md` | 后审计施工队列 Q0–Q20（含 §1 裁定记录、§4 逐批状态、§5 取证锚点、§6 收口口径、§7 变更纪律）。**它的未完成部分已重排进 `../design/roadmap/` 与 `../../specs/`**；裁定与取证事实留在本文里可查 | `docs/design/Post-Audit Roadmap v1.md` |
| `doc-governance-plan-v1.md` | 文档治理计划 v1（2026-10-17 提出并批准；含当时的成本实测底数与 D1–D7 草案） | `docs/design/DOC-GOVERNANCE-PLAN-v1.md` |
| `sandbox-execution-design.md` | 沙箱执行设计 —— 随 v2.145 权限模型重设计整目录退役，命令执行面收敛为 `shell` 单一入口 | `docs/design/archive/Sandbox Execution Design.md` |
| `icon-replacement-plan.md` | 图标替换计划（已执行完） | `docs/design/icon replacement plan.md` |
| `context-memory-workflow-report.md` | 上下文与记忆工作流程报告（过程记录） | `docs/上下文与记忆工作流程报告.md` |

## 还没搬进来的

`docs/reports/`（两份审计报告与修复台账）暂时留在原处：台账 §7 里还挂着**没结的欠账**，
要先把它们抄进路线图与阶段计划，再整体归档 —— 这一步在"路线图重写"那一刀做，别在这里留口头账。

## 旧链接怎么办

本文里的历史文件**正文内部**还写着旧路径与旧互链，按规矩不改写。
在役文档、代码注释、脚本里指向上述旧路径的链接，已在这次搬迁时逐个改到新地址；
改完由文档规则闸复跑证明"没有指向不存在文件的引用"。
