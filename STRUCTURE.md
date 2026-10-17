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
  src/                   前端（形态标准 = docs/design/Front-end Design Guidelines.md）
  src-tauri/             组装点：独立 Cargo 包、不进根 workspace（根 Cargo.toml 的 exclude）
  tests/                 vitest 用例（跑法见 AGENTS.md §2）
scripts/                 ★ 工程脚本与门禁实现（任务定义只在这里，Makefile 只转发）
.githooks/               [LOCK] pre-commit（重新克隆后需 `git config core.hooksPath .githooks`）
.github/workflows/ci.yml [LOCK] CI：改前先申请
```

## 文档

| 路径 | 属性 | 说明 |
| --- | --- | --- |
| `docs/design/` | ★ | **现在有效的设计**（规范正文 + 设计稿原件）；无固定格式，只固定位置 |
| `docs/design/DESIGN.md` | ★ [LOCK] | 内核权威设计文档：改核心模块前先读对应章节 |
| `docs/design/roadmap/` | ★ | **路线图**：只列每一阶段的简称 + 勾选标记，已完成的不写 |
| `docs/adr/` | ★ | 决策记录：`NNNN-<主题>.md`，**只增不改**（规则见 `docs/adr/0000-template.md`） |
| `specs/` | ★ | 需求规格：一阶段一目录 `specs/<编号>-<名>/spec.md`，含**可测验收标准**与**非目标** |
| `docs/history/` | — | **历史归档**（过程与记忆）：流水、批次叙述、已退役审计件；**不再追加**，索引 = `docs/history/README.md` |
| `docs/references/` | — | 外部参考笔记 |

## 产物与临时件（一律不入库）

`target/` `desktop/dist/` `plugins/`（开发态安装目录）`market/`（本地市场暂存）`tmp/`（实验与一次性脚本）。

什么该入库，一句话判据：**下一个 clone 的人（或他的 agent）手里没有它，会不会做错事？**
会 → 入库（它是契约）；不会 → 不入库（它是过程或记忆）。机器可再生产物同理。

## 待归位（G2 之后本表才与实际一致）

`docs/design/archive/`（退役与未采纳稿）、`docs/design/NEW/`（设计稿原件，兑现状态看路线图）、
`docs/design/BUG/`（工单）、`docs/reports/`（审计与台账）目前仍在旧位置；搬迁映射与索引将在
`docs/history/README.md` 给出。**本节在 G2 完成时删除。**
