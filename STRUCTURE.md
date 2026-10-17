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
| `docs/design/DESIGN.md` | ★ [LOCK] | 内核权威设计文档（唯一需求来源）：改核心模块前先读对应章节，附录 C 是内核八条纪律的完整版 |
| `docs/design/UI Design System.md` | ★ | UI 层最高权威（令牌映射 / 字号相对制 / 圆角嵌套 / 冲突裁决表） |
| `docs/design/UI shell and patterns.md` | — | 下位细则：导航模型 / 页面模式库 / 事件流向 / 弹层规则（与上一行冲突时以 Design System 为准） |
| `docs/design/Front-end Design Guidelines.md` | ★ | 前端**唯一形态标准**：同类只许一种形态 + 特例申请制 + 两道机械闸 |
| `docs/design/Context and Memory.md` · `Memory Tree Design v2.md` · `KV Prefix Cache Design v2.md` · `Memory File-ization Design.md` · `Cross-Session Recall Design v1.md` | ★ | 上下文与记忆族的规范正身（v1 两份留作对照，见下） |
| `docs/design/LLM.md` · `plugins.md` · `Plug-in packaging.md` · `Tool exposure strategy and usage statistics.md` · `Setting and Constrained Optimization.md` · `edit_replace.md` · `Organize Summary Design v1.md` | ★ | 其余各主题的现役规范（`plugins.md §一` 是"哪些是插件 / 宿主内置 / 已退役"的唯一真源） |
| `docs/design/roadmap/` | ★ | **唯一进度表**：只列未完成阶段 + 勾选 + 一句人话状态；做完的删掉去 `docs/history/` |
| `docs/design/Memory-KV Roadmap.md` | ⚠ 待并档 | 它自称"记忆树 × KV 专项唯一进度表"= **第二张进度表**，与上一行冲突；未完成项并入路线图后整份归档（挂在路线图的并档项） |
| `docs/design/NEW/` | — | 设计稿原件（含已兑现与未开工的）；状态看路线图，不靠目录名 |
| `docs/design/BUG/` · `docs/issue/` | — | 工单与缺陷清单（按目录归位，状态行须与自身处理记录一致） |
| `docs/design/ui-prototype/` | — | 静态原型 = 方向参考，**不是像素合同**；演示件禁止搬进生产码 |
| `docs/design/archive/` | — | 退役与未采纳的设计稿（还剩若干份，随设计稿归位一并搬 `docs/history/`） |
| `docs/adr/` | ★ | 决策记录：`NNNN-<主题>.md`，**只增不改**；规则与模板见 `docs/adr/0000-template.md` |
| `specs/` | ★ | 阶段计划：一阶段一目录 `specs/<编号>-<名>/spec.md`，含**可测验收标准**与**非目标** |
| `docs/history/` | 只读 | **历史归档**：流水、旧路线表、旧变更日志、已退役设计稿、已批准的计划。不再追加、不改写；索引 = `docs/history/README.md` |
| `docs/reports/` · `docs/reports/_parts/` | ⚠ 待归位 | 两份审计报告与修复台账；§7 台账里还有没结的欠账，**先抄进路线图**再整体搬 `docs/history/` |
| `docs/references/` | — | 外部参考笔记 |

## 产物与临时件（一律不入库）

`target/` `desktop/dist/` `plugins/`（开发态安装目录）`market/`（本地市场暂存）`tmp/`（实验与一次性脚本）。

什么该入库，一句话判据：**下一个 clone 的人（或他的 agent）手里没有它，会不会做错事？**
会 → 入库（它是契约）；不会 → 不入库（它是过程或记忆）。机器可再生产物同理。

**一次性脚本的去处**：能长期挡错的少数几条升进 `scripts/`，其余用完即弃（要看当初怎么验的，翻提交记录）。
临时件一律放 `tmp/`，不放仓库根目录。

