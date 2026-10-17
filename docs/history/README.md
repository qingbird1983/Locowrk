# 历史归档（只读）

> 这一层放**过程与记忆**：批次流水、旧路线表、旧变更日志、已批准的计划与审计报告。
> 规矩只有三条：**不再往里追加**、**不改写内容**、**要查当时怎么做的就翻这里或 git 记录**。
> 现在有效的东西一律不在这里 —— 现役规范看 [`../design/authority/`](../design/authority/)，
> 已退役的设计稿看 [`../design/archive/`](../design/archive/)（那也是一层只读归档，与这里并列不重复：
> 这里装过程与流水，那里装做完的设计稿），进度看 [`../roadmap/`](../roadmap/)，
> 每阶段的详细计划看 [`../../specs/`](../../specs/)，决策理由看 [`../adr/`](../adr/)，
> 全仓地图看 [`../../STRUCTURE.md`](../../STRUCTURE.md)。
>
> ⚠ 本文里"原来在哪"那一列写的是**搬迁前**的旧路径，那是历史事实，按"不改写"的规矩留着；
> 要看现在的形状请以 `STRUCTURE.md` 为准。

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
| `check-length-baseline.md` | `check_length.sh` 旧版头注原文：超限文件计数基线从 40 一路收到 22 的 16 次收紧记录。v2.209 该闸改成点名名单制，计数基线不再存在，这段历史整体搬来 | `scripts/check_length.sh` 的头注释 |
| `audit/CODE-AUDIT-v1.md` | 全仓代码审计报告 v1（分域发现清单，H/M/C/L 编号的出处） | `docs/reports/CODE-AUDIT-v1.md` |
| `audit/AUDIT-FIX-PLAN-v1.md` | 审计修复执行台账：§1–§6 是批 A–F 逐批落点，**§7 的 T1–T16 未结欠账已逐条抄进 [`specs/03-structural-debt`](../../specs/03-structural-debt/spec.md)** 之后本文才归档 | `docs/reports/AUDIT-FIX-PLAN-v1.md` |
| `audit/parts/`（六份） | 审计分区明细：`0-mechanical.md`、`1-i18n-orphans.md`、`A-root-crates.md`、`B-desktop-rust.md`、`C-frontend.md`、`D-plugins-build-docs.md` | `docs/reports/_parts/` |

审计线本身**已收口**（用户 2026-10-12 裁定「审计修复到此为止」）；留在这里是为了让"当时为什么这么判"可查，
不是为了再开一批。未结的事一律从 `specs/03-structural-debt` 进，不要从本文进。

## 这一层不放什么

不放**还没结的事**。台账里的未结欠账一律先抄进对应的 [`../../specs/`](../../specs/) 阶段计划，
原文再搬进来——所以 `audit/AUDIT-FIX-PLAN-v1.md` 的 §7 那张表在这层是"当年的登记"，
在册的活计看路线图与 spec，不看这里。

## 旧链接怎么办

本文里的历史文件**正文内部**还写着旧路径与旧互链，按规矩不改写。
在役文档、代码注释、脚本里指向上述旧路径的链接，已在这次搬迁时逐个改到新地址；
改完由文档规则闸复跑证明"没有指向不存在文件的引用"。
