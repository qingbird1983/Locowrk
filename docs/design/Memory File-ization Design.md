# Locowrk 记忆文件化设计（Memory File-ization）

> 版本：v1.0 草案（2026-10-05，用户方向口述 → 设计稿，**待用户裁定后生效为规范**）
>
> **用户方向原话记录（2026-10-05，三条）**： ①「我们的记忆树，记录存盘是要以 .md 格式存进 .Locowrk 的缓存目录的，人可查可读可写。分为全局记忆，只记录用户偏好，全局要求等。项目记忆，分为在缓存目录里建立各项目的 ID 文件夹，然后在里面再分级存储项目记忆。缓存目录里主要存储项目级的记忆。也就是上下文主要要拼装的记忆，连同全局记忆。然后在项目仓库建立 .Locowrk 文件夹。里面记录项目工作过程的记录，与项目的要求等等，这部分记忆主要适应做上下文交接用的。另外会主动要求讲这个 .Locowrk 文件夹加入 git 排除不跟踪的。」 ②「最主要，记忆不如用户提醒才记，是主动记忆。不是被动记忆。」 ③「参考 ZCode 是怎么文件化存储的；WorkBuddy 的记忆文件化存储有参考价值（闭源，网络资料）。」

---

## 0\. 定位与裁定关系

本设计**修订**《Memory Tree Design v2》§11.3 的一条裁定——原裁定「不发明第二套声明式记忆存储，[USER.md/PROJECT.md](http://USER.md/PROJECT.md) 概念映射到 memory-sqlite 键位」修订为：**memory-sqlite 保留为机器索引层，.md 文件为人类层与权威编辑面**（WorkBuddy 同款形态：官方本体 workbuddy.db SQLite + 社区生态 `.workbuddy/memory/` Markdown 层并存）。

**与 M 轨（M4 数据 → M5 裁决）正交**：M5 的输入 = 会话日志 `llm_usage`（命中率）+ 树台账（记录员成本/熔断），文件化不写这两处；M5 的动作 = 会话级缺省开关翻转与退役决策，与存储格式无关。唯一交点 = M5「记忆页开放编辑/删除捆」——文件化先落地则该功能直接写 .md，是更好的落点。两轨互不等待。

## 1\. 设计原则

1. **文件即记忆**：人可查、可读、可写；人的编辑是权威（reconcile 进机器索引，冲突以文件为准）。
2. **原文不动**：会话 `sessions/<id>.jsonl` 仍是唯一事件真相源（append-only）；树仍是指针投影，原文不复制进记忆文件。
3. **双层存储**（WorkBuddy 同款）：SQLite = 机器索引与查询层（模型工具走这里，快）；.md = 人类层（编辑、交接、git diff 之外的个人记录）。
4. **主动记忆**：模型主动记录与更新，不等用户提醒；人的角色从「提醒」变「审阅与改写」。
5. **随仓库走**：交接文件住项目仓库 `.Locowrk/`，自动引导 gitignore；全局与项目缓存记忆住 `\~/.Locowrk/memory/`。
6. **宁严勿松**：写用户仓库仅限 `.Locowrk/` 目录内；全局记忆文件不进任何项目仓库；删除全局偏好仍走记忆管理页（人的操作，v2.68 纪律延续）。

## 2\. 存储布局

### 2.1 缓存目录（上下文拼装的记忆源）

```
~/.Locowrk/memory/
├── MEMORY.md                     # 全局索引：一文件一行（ZCode 式，每会话注入）
├── global/
│   ├── prefers-pnpm.md           # 全局记忆：用户偏好、全局要求（一事实一文件）
│   └── ...
└── projects/
    └── Locowrk-8223ad13/         # 项目 ID 文件夹 = <可读名>-<fnv 短哈希>（ZCode 式 slug+哈希）
        ├── MEMORY.md             # 项目级索引
        ├── facts/
        │   └── *.md              # 项目事实（原 memory-sqlite 项目层的文件化）
        └── tree.md               # 记忆树目录+摘要（L0/L1 人类层；L2 指针留在 JSON 投影）
```

* 单文件格式（ZCode 式）：frontmatter（`name` / `description` / `metadata.type: fact|preference|decision|handoff`）+ 正文；相关记忆 `\[\[互相链接\]\]`。
* **分级规则**：全局 = 跨项目成立的偏好与要求（「永远用 pnpm」）；项目 = 只在本项目成立的事实与决定。召回 = 本项目 ∪ 全局（与 v2.68 双层语义一一对应，键位 `(workspace, key)` → `(projects/<id>/, 文件名)`）。
* 记忆树 **[tree.md](http://tree.md)**：每捆一节——`## b3 · 缓存设计讨论` + 摘要段 + 隐性指针注释（`<!-- range: 37-82, source: recorder -->`）。看原文仍走 event_range 回会话 jsonl 现场切片（原文不进 md）。

### 2.2 项目仓库内（上下文交接层）

```
<repo>/.Locowrk/
├── handoff.md                    # 当前状态摘要（模型收束后主动更新；交接主入口：进行到哪/下一步）
├── requirements.md               # 项目要求（用户为主编辑；模型只读注入）
└── notes/                        # 工作过程记录（按主题分级，吸收 workdsh .agents/notes 形态）
    ├── 2026-10-05-记忆文件化.md    #   `<日期>-<主题>.md`，固定骨架：状态 / 问题 / 决定
    └── ...
```

* notes/ 文件骨架（借 workdsh `.agents/notes` 实践——它是「项目仓库内 AI 协作过程记录」的成熟样本）：标题 + `Status: 进行中|已收束` + **问题** + **决定**；模型收束或应要求落一篇，人可读可改。
* 首次创建时：检查仓库根 `.gitignore`，无则创建，追加一行 `.Locowrk/`（幂等，已有则跳过）——用户要求「主动要求加入 git 排除」。
* 写入边界（宁严勿松）：模型的仓库写权限**仅限** `.Locowrk/` 目录内；`requirements.md` 模型只读。
* 注入：归属会话的上下文块新增交接段（`<repo_context>`：handoff 摘要 + requirements 全文；notes/ 按需 expand），与 `workspace_context` 行同族；**M1 预取纪律兼容**——文件 mtime 进预取缓存键，人改文件 = 显式变更自动重算。

### 2.3 不文件化的部分（明确边界）

* 会话原文 `sessions/\*.jsonl`：机器真相源，体积大、append-only，不搬（人的入口是树与导出）。
* 树的 L2 指针 / 整理台账 / 熔断账本：机器簿记，留 JSON 投影（ContextAssembler 组装读投影**零变化**）。
* usage / audit：纯机器数据，不动。

## 3\. 读写路径与双向同步

| 动作 | 写 .md | 写索引（sqlite/投影） |
|---|---|---|
| 模型 memory_write（含主动提取） | ✅ 追加/更新文件 | ✅ 现有路径不变 |
| 模型 memory_delete（忘记） | ✅ 删文件 + 索引行 | ✅ |
| 人直接编辑 .md | （编辑本身） | ⭕ reconcile：启动时 + memory_reconcile 命令 + 记忆页打开时——diff 文件 ↔ 索引，以文件为准回写索引 |
| 人经记忆管理页编辑/删除 | ✅ 同步写文件 | ✅ |
| 树整理收束（记录员产出 L0/L1） | ✅ 更新 tree.md 对应节 | ✅ 投影 JSON 照写 |
| 人编辑 tree.md 的标题/摘要 | （编辑本身） | ⭕ reconcile 回写投影（title_edited_by_user 置真，纪律 5 不被覆盖） |
| 模型更新 handoff.md | ✅ 仅 .Locowrk/ 内 | —（mtime 触发注入缓存键更新） |

**同步纪律**：md 是权威（人对文件的编辑永不静默覆盖）；索引是衍生（可随时由 md + 事件流重建）。冲突场景（人改了 md、模型在旧索引上又写同键）= 保留双方并以 `updated` 时间戳新者为准 + 冲突行标注（宁可见勿丢失）。

## 4\. 主动记忆机制（WorkBuddy 语义 + ZCode 机制）

| 机制 | 说明 | 缺省 |
|---|---|---|
| 提取常开 | memory.autoExtract 缺省改开——v2.68 双层隔离（未归属拒绝项目写、全局层只进显式 global）已解除「污染」顾虑；提取落 facts/（项目）与 global/（显式偏好） | 开【待裁定④】 |
| 收束交接更新 | 归属会话轮次收束后，模型主动更新 <repo>/.Locowrk/handoff.md（进展/下一步），fire-and-forget；未归属会话跳过 | 开【待裁定③】 |
| 提示词主动指引 | 注入块尾追加主动记忆指引（遇「永远/以后都」类指令主动升格 global；重要决定当场记录不等收束） | 随注入块 |
| 忘记 | 模型 memory_delete 删文件+索引行；全局层删除仍限记忆管理页（人的操作）；WorkBuddy「过期记忆污染」教训 → 记忆页按 updated 排序暴露陈旧条目 | 沿用 |

## 5\. 与现有机器的对接（不变量）

* **ContextAssembler（M4）组装读投影 JSON 零变化**——[tree.md](http://tree.md) 是编辑面/人读面，不进组装热路径（F2 可选优化：目录文本由 [tree.md](http://tree.md) 生成，语义等价）。
* **Memory-Inject / memory-sqlite 插件契约零变化**——召回仍走索引；md 层是插件背后双写的新落点。
* **M1 前缀纪律兼容**——交接段/索引进 system 的部分按会话冻结；md 编辑 = 显式变更（缓存键含 mtime）。
* **召回键位映射**：sqlite `(workspace, key)` ↔ 文件 `(projects/<id>/facts/, <key>.md)`；全局 `''` ↔ `global/`。迁移 = 首启导出（F1），sqlite 不删（机器索引长存）。

## 6\. 迁移阶段（每阶段独立可回退）

| 阶段 | 内容 | 风险 |
|---|---|---|
| F1 双写与导出 | 布局落地：memory_write/整理收束双写 .md；首启把存量 sqlite 事实导出 global/ 与 projects/\*/facts/、树 L0/L1 导出 tree.md；组装零变化 | 低（纯增量，删 md 目录零损失） |
| F2 人的编辑面 | reconcile（人编辑 md 生效）+ 记忆页「打开文件夹」（reveal_in_explorer 现成）+ <repo>/.Locowrk/ 引导（gitignore + handoff/requirements 注入组装） | 中（同步逻辑） |
| F3 主动记忆 | autoExtract 缺省开 + handoff 收束更新 + 提示词指引 | 低（开关可退） |

## 7\. 可调项总表（§ 可配置原则）

| 项 | 缺省 | 层级 |
|---|---|---|
| 记忆文件化总开关 | 开（关 = 回纯 sqlite，md 不再读写） | 用户（记忆设置） |
| autoExtract（主动提取） | 改开【待裁定④】 | 用户（记忆设置） |
| handoff 收束更新 | 开【待裁定③】 | 用户（记忆设置） |
| reconcile 时机 | 启动 + 记忆页打开 + 手动命令 | 开发者 |
| 交接注入（requirements/handoff 段） | 开 | 用户（记忆设置） |
| 项目文件夹命名 | 可读名-短哈希【待裁定②】 | — |

## 8\. 开放决策点（待用户裁定）

1. **事实记忆存储形态**：推荐**双层双写**（md 权威编辑面 + sqlite 机器索引，WorkBuddy/ZCode 生态同款）——memory_read/search/undo 插件生态不动、风险小。备选 = 全迁 md、sqlite 退役（更纯粹，但召回/undo/管理页全重写，不建议本阶段）。
2. **项目文件夹命名**：推荐 `<可读名>-<fnv 短哈希>`（ZCode 式，可读且防重名）；备选纯可读名（重名冲突需处理）。
3. **[handoff.md](http://handoff.md) 收束自动更新缺省**：推荐开（写仓库仅限 `.Locowrk/` 内 + 首次创建 toast 告知 + 开关可关）。
4. **autoExtract 缺省改开**：推荐改开（主动记忆的核心；v2.68 隔离已解污染顾虑）。

## 9\. 参考资料

* **ZCode（本地开源，F:\\Warehouse\\Github\\ZCode）**：`packages/services/src/memory/memoryService.ts`——`<dataRoot>/cli/memories/projects/<slug>-<16hex>/memory/\*.md` + `MEMORY.md` 索引；一事实一文件 + frontmatter；模型自主写（主动记忆）+ 索引每会话注入 + 按需召回；symlink/大小写别名防护。
* **WorkBuddy（闭源，网络资料）**：官方记忆 = 「自动从对话提取偏好与习惯，定期更新长期留存」，用户可**查看/编辑/导入/忘记**；官方本体 `workbuddy.db`（SQLite+WAL），社区生态流行 `.workbuddy/memory/` Markdown 目录层（BFLABS Memory「用户级 [MEMORY.md](http://MEMORY.md) + 项目 .workbuddy/memory/」、DSH Memory Palace「可编辑 Markdown 保存跨会话记忆、日志与摘要」）；社区教训：**过期记忆会污染新任务**，要定期检查删除。来源：[awesome-workbuddy RESOURCES.md](https://github.com/sandbaseai/awesome-workbuddy/blob/main/RESOURCES.md)、[WorkBuddy 从入门到精通（什么值得买）](https://post.smzdm.com)、[腾讯云开发者社区 WorkBuddy 评测](https://cloud.tencent.com/developer)。
* **workdsh（开源，[gitee.com/techflag/workdsh](https://gitee.com/techflag/workdsh) / GitHub 同步）**：WorkBuddy 式桌面工作台（DeepSeek Harness 插件生态，MIT）。**核查结论：主仓库无记忆模块实现**（全仓 grep 无 memory 功能，记忆仍在闭源上游 workbuddy.db）——它可借鉴的不是记忆本体，而是 **`.agents/notes/` 交接笔记形态**：按主题分文件（`<日期>-<主题>.md`）+ `implemented/proposed` 状态目录 + 固定骨架（Status/Problem/Decision）+ 中英双语——本设计 §2.2 notes/ 直接吸收。