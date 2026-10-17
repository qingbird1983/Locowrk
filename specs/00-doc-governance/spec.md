# 00-文档规范治理

- 状态：已完成（2026-10-17，G1–G5 五刀；下一步走向由用户指定，不在本阶段里推）
- 负责人：青鸟（决定方向） / Qoder（执行）
- 日期：2026-10-17
- 出处 / 上游：用户 2026-10-17 指令 —— 全部依照 `F:\Users\Warehouse template` 的方向；
  旧文档约束废除，历史归类加索引，未来工作按新规则重排；当时的成本底数与提案见
  [`docs/history/doc-governance-plan-v1.md`](../../docs/history/doc-governance-plan-v1.md)

## 目标

解决一件事：**记一笔变更比做这笔变更还费时间**。治理前实测（v2.208 那一版）：写一次收尾要动
10 个文件、同一件事在 6 份文档里被写 11 遍、变更日志一版一行最长 13 KB 读不动、
入口说明里 6 KB 是进度而不是规则。治理后：同一件事只写一处，收尾只动一张进度表加一次提交。

## 验收标准（必须可测：跑什么命令、看到什么结果）

- [x] AC-1 跑 `make doc`：全绿，末行输出 `DOC_FAILED=0`；八条判据逐条能解释（入口厚度 / 地图 / 决策记录 / 阶段计划 / 长行 / 旧规矩用词 / 历史索引 / 断链）
- [x] AC-2 故意把 `AGENTS.md` 加到 101 行再跑 `make doc`：**必须报红**并点名 AGENTS.md（改回后转绿）—— 证明这条判据不是摆设
      · 实测 100 行绿 / 101 行红；顺手修掉了判据自己的读数错（`split('\n').length` 让以换行结尾的文件多报 1 行，103 行的文件报成 104）
- [x] AC-3 在 `docs/design/roadmap/README.md` 里加一段 3 KB 的超长行再跑 `make doc`：**必须报红**（证明"不许再来万字长行"真的在管）
- [x] AC-4 把 `docs/history/` 里任一文件从 `docs/history/README.md` 的表里删掉一行再跑 `make doc`：**必须报红**（历史有索引、不漏件）
- [x] AC-5 在役文档里的相对链接**必须指得到真实文件**，读数 = 0（判据 R8）
      · 这条原本是"跑一次确认"的人工承诺，闸里**并没有**它 —— G5 把它补成了真判据，
        补完当场红出 4 条：3 条是搬迁留下的旧址（`DESIGN.md` / `plugins.md` 指向已搬走的
        ROADMAP/CHANGELOG），1 条是本轮我自己把相对路径深度写错（`../specs/...` 从根目录出发）
      · 代码注释与脚本里的文档路径引用**仍是人工核对**：本批全仓 grep 过一遍（在役面 1 处，已修）。
        没做成判据的原因是形态太杂、假红风险高，硬凑一条会训练人去绕过它 —— 记为遗留，见下
- [x] AC-6 你在根目录打开 `STRUCTURE.md`：能找到"某个文件能不能动"的答案；打开 `AGENTS.md`：≤100 行且读完知道下一步该看哪（实测 98 行）
- [x] AC-7 你随便挑一个已完成阶段（例如记忆看板），在 `docs/history/README.md` 里能找到它的旧文件与它原来在哪

## 非目标

- **不改写历史**：旧变更日志、旧路线表、旧进度流水一律逐字原样搬走，不重写、不折行、不转成决策记录
- 不做记忆与上下文的重构设计（那是阶段 01）
- 不追求照抄模板的每一个文件（模板里没有的约束我们照旧保留：中英成对、插件热插拔、形态标准等）
- 不加新闸门来管代码；代码规则原样保留，只搬位置不改语义

## 设计要点

一句话：**一份事实只有一个地方写**，其余地方给指针。有长期影响的取舍另写一篇决策记录（只增不改）。
细则见 [`CONTRIBUTING.md`](../../CONTRIBUTING.md) 的「一版变更要写哪几处」。

## 计划

1. 立骨架：地图、提交约定、一条命令 + 任务实现、决策记录模板、阶段计划模板 ✅
2. 历史归位：流水类文件整体搬进 `docs/history/` + 一份索引（含旧路径 → 新路径） ✅
3. 进度表重写：只列未完成阶段 + 勾选 + 人话状态 ✅
4. 入口文档重写：`AGENTS.md` ≤100 行（实测 98）、`README.md` 说人话并纠正过期数字 ✅
5. 审计件归位：§7 的 T1–T16 未结欠账先抄进 `specs/03-structural-debt`，两份报告与六份分区明细
   整体搬进 `docs/history/audit/` ✅
   · **设计稿原件不搬**（`docs/design/NEW/` 留着）：已兑现与未开工的稿子混在同一目录，
     靠目录名判状态会造假信号；状态一律看路线图那一行。这是取舍不是漏做。
6. 检查脚本合并修订：逐条给"留 / 改 / 合 / 删"并说明为什么卡、卡了会怎样 ✅ → 成文为
   [ADR-0002](../../docs/adr/0002-gates-rewritten-to-template.md)

## 任务

- [x] 骨架四件 + 两个模板
- [x] `docs/history/` 与索引
- [x] 新文档规则闸 `scripts/check-docs.mjs`，旧窗口闸退役
- [x] `AGENTS.md` / `README.md` 重写
- [x] 审计件归位（设计稿按上面第 5 条的取舍留在原处）
- [x] 其余检查脚本合并修订（删编译器已承重的档 / 长度闸改名单制 / 断链判据补上 / 闸清单收进一处）
- [x] `make check` 全绿（2026-10-17 实跑 rc=0：根 515 · 桌面 579 · 前端 104／13 files · 闸全 0）
- [x] 路线图「文档规范治理」那一行：做完已从路线图删掉，旧叙述留在 `docs/history/` 与本阶段计划

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `STRUCTURE.md` `CONTRIBUTING.md` `Makefile` `scripts/tasks.sh` `docs/adr/` `specs/` `docs/history/` `docs/design/roadmap/` | 新建 |
| `AGENTS.md` `README.md` | 重写 |
| `docs/design/ROADMAP.md` `CHANGELOG.md` `INDEX.md` `Post-Audit Roadmap v1.md` `docs/design/archive/*` | 搬进 `docs/history/` |
| `docs/reports/`（两份报告 + 六份分区明细） | 搬进 `docs/history/audit/`，目录消失 |
| `scripts/check-doc-budget.mjs` | 退役删除，由 `scripts/check-docs.mjs` 接管 |
| `scripts/check_length.sh` | 计数基线 → 点名名单；16 次收紧史搬进 `docs/history/check-length-baseline.md` |
| `scripts/check-split-equivalence.mjs` | 删掉 rustc 已承重的四档（657 → 502 行）；登记表重放撤出常驻门禁 |
| `scripts/check-style.mjs` | "一名两义"合掉：两个档同语义，全量档不再恒绿 |
| `scripts/check-single-source.mjs` | 新建：跨文件"同一事实只许一个出处"这类对账的**唯一收口处** |
| `.githooks/pre-commit` `.github/workflows/ci.yml` | 闸清单收进 `tasks.sh` 一处；CI 改成一行调用；头注不再复述每条闸挡什么 |

## 风险与回滚

- 风险 1：搬走文件让在役文档与代码注释里的链接落空 → 每条搬迁后跑一次"指向不存在文件的引用 = 0"，
  只修在役面的链接，历史正文按规矩不动。
- 风险 2：新文档闸被写成"永远绿"的摆设 → 靠 AC-2/3/4 三条**必须报红**的核对来证伪。
- 风险 3：进度表变薄以后你看不到细节 → 细节进提交记录；已交付阶段的旧叙述全部留在 `docs/history/`。
- 挂账 / 遗留：①`tmp/` 里 **283 份**历史一次性判据脚本的逐条处置（作废或删除）仍在台账
  [`specs/03-structural-debt`](../03-structural-debt/spec.md) 的 T8；本批已经把"该常驻的那一类"
  收成一处（`check-single-source.mjs`），所以这条从此**只减不增**。
  ②代码注释与脚本里的文档路径引用是**人工核对**，不是机器判据（理由写在 AC-5）。
  ③路线图「文档规范治理」那一行还挂着 —— 本阶段做完就该从路线图删掉。
- 回滚：每刀一个提交，`git revert` 单刀即可；搬迁用的 `git mv`，改名历史仍可追。
