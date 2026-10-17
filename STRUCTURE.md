# 仓库蓝图（STRUCTURE.md）

> **本仓库唯一的地图**，不要另建一份。规则在 [`AGENTS.md`](AGENTS.md)，本文件只回答"这个文件是干什么的、能不能动"。
> **新增 / 移动 / 删除文件后同步更新本文件** —— 一张说谎的地图比没有地图更糟。
> 本树是**当前实际状态**，不写愿望。若本文件与仓库不符，相信仓库，并修正本文件。

只有两个标记：

- `★` **唯一事实源**：改这里，其他地方只引用、不复制
- `[LOCK]` **禁区**：改动前必须向人说明并获批准

## 根目录

```
AGENTS.md                ★  [LOCK]  agent 入口：命令 / 硬规则 / 禁区 / 工作流 / 收口清单
STRUCTURE.md             ★         本文件：全仓文件蓝图
README.md                          给人看的入口
CONTRIBUTING.md                    提交与评审约定 + 一版变更写哪几处
Makefile                     命令入口薄封装（仅转发到 tasks.sh）
Cargo.toml / Cargo.lock      [LOCK]（lock 由 cargo 更新，禁止手工编辑）
```

## 工程实现

```
crates/                  ★ 内核 crate（依赖单向图见 AGENTS.md §4；边界改动 = 设计错误）
  plugin-api/            [LOCK] 冻结 ABI：只许把字段追加到结构体末尾，改已有签名是 breaking
  cordis-core/ kernel-agent/ plugin-host/ plugin-service/
  session-store/ memory-tree/ kernel-bin/
plugins-src/             内置插件源码（workspace 成员；版本真源 = scripts/build_plugins.sh 内 PLAN 清单）
desktop/                 Tauri 2 + Vue 3 桌面端
  src/                   前端（形态标准 = docs/design/authority/Front-end Design Guidelines.md）
  src-tauri/             组装点：独立 Cargo 包、不进根 workspace（根 Cargo.toml 的 exclude）
  tests/                 vitest 用例（跑法见 AGENTS.md §2）
scripts/                 ★ 工程脚本与门禁实现（任务定义只在这里，Makefile 只转发）
.githooks/               [LOCK] pre-commit（重新克隆后需 `git config core.hooksPath .githooks`）
.github/workflows/ci.yml [LOCK] CI：改前先申请
```

## 门禁

`scripts/tasks.sh` 是**任务与闸清单的唯一出处**（★）。Makefile 只转发、CI 只调 `sh scripts/tasks.sh gates`、
pre-commit 只按暂存面挑要跑哪几条 —— 三处都不抄清单（v2.209 之前 CI 手抄过一份，漏了两道闸且因为从未 push 而无人发现）。

| 档 | 命令 | 跑什么 | 什么时候用 |
| --- | --- | --- | --- |
| 提交档 | `git commit`（自动） | 按暂存面触发的结构闸 + 前端测试 | 每次提交，pre-commit 拦住 |
| 秒级档 | `make quick` | 全部结构闸 + 闸的自证 + 前端形态 | 改完一轮就想确认时 |
| 全量档 | `make check` | 上者 + 类型 + clippy + 三处测试 | 一批收口之前，最后一次编辑之后 |
| 自证档 | `make selftest` | 给判据施内存变异，它必须报红 | 改过闸本身之后 |
| 一次性 | `make ledger LEDGER=<名>` | 重放某刀的拆分等价证明 | 只有重做那一刀时才跑；**不在常驻门禁里**（历史原文 vs 当前产物必然漂） |

## 文档

| 路径 | 属性 | 说明 |
| --- | --- | --- |
| `docs/roadmap/` | ★ | **唯一进度表**（2026-10-17 从 design 里提出来放 docs 根）：只列未完成阶段 + 勾选 + 一句人话状态；做完的删掉 |
| `docs/design/authority/` | ★ | **各主题现在有效的那一份规范**（15 份）。判据：改了代码必须回来同步它。清单、层级与冲突裁决顺序见 `docs/design/authority/README.md` |
| `docs/design/authority/DESIGN.md` | ★ [LOCK] | 内核权威设计文档（唯一需求来源）：改核心模块前先读对应章节，附录 C 是内核八条纪律的完整版 |
| `docs/design/authority/UI Design System.md` | ★ | UI 层最高权威（令牌映射 / 字号相对制 / 圆角嵌套 / 冲突裁决表） |
| `docs/design/authority/Front-end Design Guidelines.md` | ★ | 前端**唯一形态标准**：同类只许一种形态 + 特例申请制 + 两道机械闸 |
| `docs/design/authority/`（其余 12 份） | ★ | 上下文与记忆族 · `LLM.md` · `plugins.md`（§一 是"哪些是插件 / 宿主内置 / 已退役"的唯一真源）· `Plug-in packaging.md` · 工具暴露统计 · 设置与约束 · `edit_replace.md` · 整理摘要 |
| `docs/design/`（散在根上的 7 份） | — | **还有用但还不是正身**：未开工或部分开工的设计稿 + 下位细则（`UI shell and patterns.md`）。其中 `Memory-KV Roadmap.md` 是 ⚠ 待并档的第二张进度表，未完成项并进 `docs/roadmap/` 后归档 |
| `docs/design/archive/` | 只读 | **已兑现 / 已退役 / 被新版本取代的设计稿**（15 份）。不再追加、不再改写；索引 = `docs/design/archive/README.md`（含"首行状态滞后"这个坑的说明） |
| `docs/issue/` | — | 工单与缺陷清单（含从 design 里并过来的三份）；状态行须与自身处理记录一致 |
| `docs/prototype/` | — | 静态原型 = 方向参考，**不是像素合同**；演示件禁止搬进生产码 |
| `docs/references/` | — | 外部参考笔记 |
| `docs/adr/` | ★ | 决策记录：`NNNN-<主题>.md`，**只增不改**；规则与模板见 `docs/adr/0000-template.md` |
| `specs/` | ★ | 阶段计划：一阶段一目录 `specs/<编号>-<名>/spec.md`，含**可测验收标准**与**非目标** |
| `docs/history/` | 只读 | **历史归档**：流水、旧路线表、旧变更日志、已批准的计划。不再追加、不改写；索引 = `docs/history/README.md` |
| `docs/history/audit/` 与 `docs/history/audit/parts/` | 只读 | 审计报告 + 修复台账 + 六份分区明细。§7 的 T1–T16 未结欠账**已先抄进** [`specs/03-structural-debt`](specs/03-structural-debt/spec.md) 再归档到这里 |

文档只有四个去处，别新建第五个：**现役规范**进 `docs/design/authority/`，**还没做的想法**放 `docs/design/` 根，
**做完的**进 `docs/design/archive/`，**过程与流水**进 `docs/history/`。进度只在 `docs/roadmap/` 一行里。

## 产物与临时件（一律不入库）

`target/` `desktop/dist/` `plugins/`（开发态安装目录）`market/`（本地市场暂存）`tmp/`（实验与一次性脚本）。

什么该入库，一句话判据：**下一个 clone 的人（或他的 agent）手里没有它，会不会做错事？**
会 → 入库（它是契约）；不会 → 不入库（它是过程或记忆）。机器可再生产物同理。

**一次性脚本的去处**：能长期挡错的少数几条升进 `scripts/`，其余用完即弃。临时件一律放 `tmp/`，不放仓库根目录。
`tmp/` 不入库 ⇒ 它里面的东西**删了就永久消失**（git 历史里从来没有过它）：要留的结论必须在清理前落进
`scripts/`、提交正文或文档正文。2026-10-17 台账 T8 收口时按这条办，因此历史文档与 `scripts/split-ledger/`
里那些"脚本 = `tmp/…`"的溯源句从此**只作史料、不保证可追**。

