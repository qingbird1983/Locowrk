# Locowrk 记忆文件化设计（Memory File-ization）

> 版本：**v1.1 生效规范**（2026-10-18 升版）。v1.0 是 2026-10-05 的方向草案，长期停在"待用户裁定"，
> 而 `crates/plugin-api/src/memlayout/mod.rs:1`、`desktop/src-tauri/src/memory_files.rs:1` 与
> `docs/design/Memory-KV Roadmap.md:9`/`:174` 三处早就按「v1.1」实现与引用 —— **实现先于标准升版**
> 就是这次漂移的根因；本版把标准补到已被引用的那个版本号上，并按 [`ADR-0007`](../../adr/0007-memory-flat-files-context-llm-compaction-primary.md) 扳正真源方向。
>
> **用户方向原话记录（2026-10-05，三条）**： ①「我们的记忆树，记录存盘是要以 .md 格式存进 .Locowrk 的缓存目录的，人可查可读可写。分为全局记忆，只记录用户偏好，全局要求等。项目记忆，分为在缓存目录里建立各项目的 ID 文件夹，然后在里面再分级存储项目记忆。缓存目录里主要存储项目级的记忆。也就是上下文主要拼装的记忆，连同全局记忆。然后在项目仓库建立 .Locowrk 文件夹。里面记录项目工作过程的记录，与项目的要求等等，这部分记忆主要适应做上下文交接用的。另外会主动要求讲这个 .Locowrk 文件夹加入 git 排除不跟踪的。」 ②「最主要，记忆不如用户提醒才记，是主动记忆。不是被动记忆。」 ③「参考 ZCode 是怎么文件化存储的；WorkBuddy 的记忆文件化存储有参考价值（闭源，网络资料）。」
>
> **v1.1 勘误表（改了什么，逐条可查）**
>
> | 节 | v1.0 草案说的 | v1.1 生效的 | 依据 |
> |---|---|---|---|
> | §8 决策点 1 | 推荐**双层双写**（md 权威编辑面 + sqlite 机器索引），"全迁 md、sqlite 退役"列为不建议本阶段的备选 | **采纳那条备选**：md = 唯一真源，无第二存储；sqlite 存量一次性导出后原地保留不读不删 | ADR-0007 第 1 条 + 被否决表（双真源正是"记忆始终是混乱的"根因） |
> | §1 原则 1/3 | "reconcile 进机器索引、冲突以文件为准" + "双层存储" | reconcile 这个概念**整体消失**（只有一个真源就无处对账）；§3 那张"写索引 ✅"表作废 | ADR-0007 第 1/6 条 |
> | §2.1 frontmatter | `metadata.type: fact\|preference\|decision\|handoff` | 四类型改 `user\|feedback\|project\|reference`（Qoder/agent-sdk 硬编码枚举），`tags` 降级为自由标签；`feedback`/`project` 正文带 `Why:` 与 `How to apply:` | ADR-0007 第 2 条 |
> | §2.1 布局 | 全局索引 `MEMORY.md` 画在根上 | 索引坐在 `global/MEMORY.md`（与 `scope_dirs`(:264-275) 实况一致），未归属键位另有 `_unaffiliated/` | 2026-10-18 现状取证，决策人裁定"布局照现状" |
> | §2.1 索引尺寸 | 只说"一文件一行" | **钳落在写侧**：`refresh_index` 出 `MEMORY.md` 时截到 200 行 / 25_000 字符并在末行写 WARNING；注入侧不再现算 | 人打开看到的就该是模型看到的那份（同 §2.2 替换语义一条物理定律） |
> | §5 不变量 | "Memory-Inject / memory-sqlite 插件契约零变化——召回仍走索引" | 作废：召回改读 md 与索引，插件契约要改（工具名与参数面不动，实现改道） | ADR-0007 第 5/16 条 |
> | §8 决策点 3/4 | handoff 收束更新、autoExtract 缺省开 | 这两条随 organize 管线退役一并作废（记录员整面下线，交接层形态改两文档制） | ADR-0007 第 16/17 条 |
> | §4 | 主动记忆 = 整理收束顺带产事实卡 | 目标形态 = 回合收束后**增量**抽取（只取游标之后的新消息 + 前次产出快照去重）；**不在 S1 范围**，另立批 | ADR-0007 第 5 条；范围界定见 [`specs/05`](../../../specs/05-memory-data-plane-s1/spec.md) 非目标 |

---

## 0. 定位与裁定关系

本设计取代《Memory Tree Design v2》§11.3「不发明第二套声明式记忆存储，`USER.md`/`PROJECT.md`
概念映射到 memory-sqlite 键位」那条裁定，并把 v1.0 推荐的"memory-sqlite 保留为机器索引层"
**再往回扳一步**：md 文件既是人类层也是**唯一真源**，机器索引层不再存在。记忆树与整理管线
（organize）整体退役，交接层 `<repo>/.Locowrk/` 改两文档制（见 §2.2）——完整取舍与被否决的
方案表在 `docs/adr/0007`，本文不复制理由。

**与 M 轨（M4 数据 → M5 裁决）正交**：M5 的输入 = 会话日志 `llm_usage`（命中率）+ 树台账，
文件化不写这两处；M5 的动作 = 会话级缺省开关翻转与退役决策，与存储格式无关。

## 1. 设计原则

1. **文件即记忆，且是唯一的记忆**：人可查、可读、可写；人的编辑就是权威写入，不存在"回写
   某个索引"这一步。
2. **原文不动**：会话 `sessions/*.jsonl` 仍是唯一事件真相源（append-only）；记忆文件里只放
   事实本身，原文靠 event_range 回日志现场切片。
3. **无第二存储**：不建 sqlite / JSON 投影 / 内存索引中的任何一份作为真源。召回慢就修文件读，
   不加层——多加一层就多一个说谎的地方（v1.0 那版推荐的双写正是这么坏的）。
4. **主动记忆**：模型主动记录与更新，不等用户提醒；人的角色是审阅与改写。
5. **随仓库走**：交接文件住项目仓库 `.Locowrk/`，自动引导 gitignore；全局与项目记忆住
   `~/.Locowrk/memory/`。
6. **宁严勿松**：写用户仓库仅限 `.Locowrk/` 目录内；全局记忆文件不进任何项目仓库；
   删除 / 覆盖数据根任何内容前单独向人确认。

## 2. 存储布局

### 2.1 缓存目录（上下文拼装的记忆源）

```
~/.Locowrk/memory/
├── global/                       # 全局层 = 键位 workspace 为空
│   ├── MEMORY.md                 # 全局索引：一文件一行，每会话注入；写侧硬钳 200 行 / 25_000 字符
│   └── prefers-pnpm.md           # 全局事实：跨项目成立的用户偏好、全局要求（一事实一文件）
├── projects/
│   └── Locowrk-8223ad13/         # 项目 ID 文件夹 = <可读名>-<fnv 短哈希>（§8 决策点 2 已兑现）
│       ├── MEMORY.md             # 项目级索引（坐在项目文件夹根）
│       ├── facts/
│       │   └── *.md              # 项目事实
│       └── tree.md               # 记忆树人类层（随 organize 退役，S4 停写）
└── _unaffiliated/                # 未归属键位兜底（'' 不是项目，不建进 projects/）
```

* 单文件格式：frontmatter（`name` / `description` / `tags` / `metadata{type, source, workspace, updated}`）
  + 正文。`metadata.type` ∈ `user | feedback | project | reference`（旧文件读回 `fact` = 未分类，
  不重写存量）。`feedback` 与 `project` 正文带 `Why:` 与 `How to apply:` ——知道为什么才判得了边界情况。
* **`tags` 必须单独一行，不能靠 `description` 反推**：`description` 装的是"tags，tags 空则正文首行"
  （渲染侧有意做的 hook），是个**有损**字段——从它读不回"这是标签还是正文第一句"。真源一旦是文件，
  管理页的列举过滤与 `memory_search` 的 tags 匹配都只能从 `tags:` 这行取，所以它是一等字段。
  旧文件没这行 ⇒ 读回空串（不回填、不重写存量）。
* **分级规则**：`user` / `feedback` 缺省落全局，`project` / `reference` 缺省落项目，显式
  `scope` 可覆盖；映射由 `plugin-api::memlayout` 的一个纯函数算，模型只报 type、不自己算目录
  （记忆工具有宿主与插件两条通路，放任何一侧都算不出另一侧的落点）。
* 相对日期一律转绝对（"下周三"进文件必须是某个 `YYYY-MM-DD`）。
* **排除清单**（写进工具说明与抽取提示词，不配进记忆的）：代码可推导的（模式 / 约定 / 架构 /
  路径 / 项目结构）、git 查得到的（历史 / 谁改了什么）、`AGENTS.md` 与 `.Locowrk/` 已写的、
  临时任务状态（改了哪些文件、分支与 commit 态、测试过没过、重试）。用户明说"记住"这类内容，
  先反问"哪里令人意外"——那才是值得记的部分。

### 2.2 项目仓库内（上下文交接层，两文档制）

```
<repo>/.Locowrk/
├── MEMORY.md                     # 现状 = 交接主入口：按主题分节、原地替换、每条带日期戳、硬顶 200 行 / 10KB
├── journal/<YYYY-MM-DD>.md       # 流水：一天一文件，当天可写、昨日封卷不可变
└── requirements.md               # 项目要求（用户为主编辑；模型只读）
```

* 旧形态（`handoff.md` 单文件 + `notes/` 按主题）实测会无限膨胀——病根是把"流水"写进"现状文档"：
  只追加不替换、没有尺寸顶。两文档制的纪律 = **原地替换** + **按天封卷**，超顶逼蒸馏。
* 首次创建时检查仓库根 `.gitignore`，无则创建、追加一行 `.Locowrk/`（幂等，已有则跳过）。
* 写入边界（宁严勿松）：模型的仓库写权限仅限 `.Locowrk/` 内；`requirements.md` 模型只读。
* 注入：`<repo_context>` 带 `MEMORY.md` 正文 + `requirements.md` 提醒行；journal 不注入、按需读。
  **M1 预取纪律兼容**——文件 mtime 进预取缓存键，人改文件 = 显式变更自动重算。

### 2.3 不文件化的部分（明确边界）

* 会话原文 `sessions/*.jsonl`：机器真相源，体积大、append-only，不搬。
* 熔断账本、整理台账一类机器簿记：留原处，不冒充记忆（organize 退役后大部分消失）。
* usage / audit：纯机器数据，不动。

## 3. 读写路径

真源只有一份 ⇒ 只有一条写路径：**写文件 + 刷该层索引**，失败就是失败（上抛，不 `warn` 降级）。

| 动作 | 落盘 |
|---|---|
| 模型 `memory_save`（含主动抽取） | 写 `<层>/<stem>.md` + 刷该层 `MEMORY.md`；同键查重提示 |
| 模型 `memory_delete`（忘记） | 删文件 + 索引行即真忘；管理页删除走 `obsolete/` 软删，清空才算真删（仍要点头） |
| 人直接编辑 .md | 就是权威写入，无需任何同步动作；下一轮请求生效（mtime 进缓存键） |
| 人经记忆管理页编辑 / 删除 | 同一批文件操作，不开第二条写路径 |
| 交接层现状 / 流水更新 | 见 §2.2（原地替换 / 按天封卷） |

冲突保护只剩一种场景值得处理：机器准备覆盖一个**被人改过**的文件。做法沿用现实现
（`memlayout/sync.rs` 的 `stash_human_edit` + mtime 标记）= 先备份再写，双方都留，宁可见勿丢失。

## 4. 主动记忆机制

> **v2.136 修订（用户裁定，2026-10-09）**：三机制中 ①提取常开（autoExtract）与 ②收束交接更新
> （handoff 后台改写）双双退役——项目记忆由当前工作的**主模型**自己在工作中读 / 写 / 改，软件不
> 代劳，只在 `<repo_context>` 静态提醒段给出节点提醒钩子。③提示词主动指引保留。experiences 向量
> 经历层一并退役（唯一写入方是 autoExtract 的 episode）。

v1.1 的生效形状：

| 机制 | 说明 | 缺省 |
|---|---|---|
| 工具写入 | `memory_save` / `read` / `search` / `delete` 四枚操作文件；用户明说"记住 / 忘掉"立即执行不排队 | 开 |
| 回合收束增量抽取 | 按游标只取上次之后的新消息，带前次已产出快照去重，一次性并行写完；"创建零条记忆是正常的"。**不在 S1 范围**（要 LLM 通道，随 S2 之后另立批） | 待立批 |
| 静态提醒钩子 | 开场读 `requirements.md` + 交接层 `MEMORY.md`；现状实质变化时主模型自行更新（v2.136 裁定沿用） | 开 |
| 忘记 | 删文件 + 删索引行；验证只看文件系统（文件不在 = 不在）；社区教训"过期记忆污染新任务" ⇒ 管理页按 `updated` 排序暴露陈旧条目 | 沿用 |

## 5. 与现有机器的对接

* **ContextAssembler 组装读投影**：树目录段随树退役移除，注入面改吃索引指针行（`specs/01` C 节）。
* **记忆工具**：插件 `memory-sqlite` 那 6 枚工具**名字与参数面不动**（提示词与审批面才不会跟着漂），
  内部实现改道文件；插件本身按 ADR-0007 第 16 条在 S4 退役，搬不搬进宿主单独点头。
* **M1 前缀纪律兼容**：索引文本按会话冻结；md 编辑 = 显式变更（缓存键含 mtime）。
* **存量迁移**：首启导出（幂等，已有文件一律不动、人的编辑永不被覆盖），之后 sqlite 不读不删。

## 6. 施工阶段

本篇是标准；阶段计划与可测验收在 [`specs/01-memory-context-redesign/spec.md`](../../../specs/01-memory-context-redesign/spec.md)（总表）与 [`specs/05-memory-data-plane-s1/spec.md`](../../../specs/05-memory-data-plane-s1/spec.md)（S1 数据面，三刀）。v1.0 那张 F1/F2/F3 迁移表已被取代：F1 双写、F2 reconcile 都已落地过，正是本刀要扳的存量。

## 7. 可调项总表（可配置原则）

| 项 | 缺省 | 层级 |
|---|---|---|
| 记忆文件化总开关（`memory.files`） | 开（v1.1 起"关"的语义要重定：md 已是唯一真源，关掉它 = 记忆功能整体不可用，不是"回纯 sqlite"） | 用户（记忆设置） |
| 全局记忆 / 项目记忆 / 项目交接层三枚行为开关 | 各开各的；关 = 该层不读、不更新、不注入 | 用户（记忆设置） |
| 索引硬顶 | 200 行 / 25_000 字符（写侧）；交接层 200 行 / 10KB | 开发者 |
| reconcile 时机 | **整项作废**（无第二存储可 reconcile） | — |
| 交接注入（`MEMORY.md` / `requirements.md` 段） | 开 | 用户（记忆设置） |
| 项目文件夹命名 | `<可读名>-<fnv 短哈希>`（已兑现） | — |

## 8. 裁定状态

1. ~~双层双写~~ → **md 唯一真源**（2026-10-17 裁定，见 ADR-0007；sqlite 存量一次性导出，
   原地保留不读不删）。
2. 项目文件夹命名 `<可读名>-<fnv 短哈希>` → **照推荐已落地**，本条关闭。
3. handoff 收束自动更新 → 随 organize 退役作废，交接层改两文档制（§2.2）。
4. autoExtract 缺省开 → 随 organize 退役作废，替换为"回合收束增量抽取"，另立批。

## 9. 参考资料

* **ZCode（本地开源，`F:\Warehouse\Github\ZCode`）**：`packages/services/src/memory/memoryService.ts`
  ——`<dataRoot>/cli/memories/projects/<slug>-<16hex>/memory/*.md` + `MEMORY.md` 索引；一事实一文件
  + frontmatter；模型自主写（主动记忆）+ 索引每会话注入 + 按需召回；symlink / 大小写别名防护。
  CLI 侧还有 `apps/zcode-cli/packages/core/src/memory/`（`directory.ts` 布局、`extraction.ts` 抽取、
  `dream.ts` 后台整理通道——最后这个正是本设计非目标里"不预造机制"所指的是物）。
* **Qoder CN（本机逆向，2026-10-17 取证）**：`~/.qoder-cn/memory/` 全局 55 个事实文件 +
  `projects/<项目>/` 两层，四类型枚举硬编码在官方 agent-sdk 提示词里；无向量层、无机器索引层。
* **WorkBuddy（闭源，网络资料）**：官方记忆 = 自动从对话提取偏好与习惯，用户可查看 / 编辑 /
  导入 / 忘记；官方本体 `workbuddy.db`（SQLite+WAL），社区生态流行 `.workbuddy/memory/` Markdown 层；
  本机实物 `qingbird-git/.workbuddy`（7 文件 85KB）给的是**两文档制**。社区教训：**过期记忆会污染
  新任务**，要定期检查删除。来源：[awesome-workbuddy RESOURCES.md](https://github.com/sandbaseai/awesome-workbuddy/blob/main/RESOURCES.md)、[什么值得买 WorkBuddy 介绍](https://post.smzdm.com)、[腾讯云开发者社区评测](https://cloud.tencent.com/developer)。
* **workdsh（开源，[gitee.com/techflag/workdsh](https://gitee.com/techflag/workdsh)）**：核查结论 =
  主仓库无记忆模块实现；可借鉴的不是记忆本体，而是 `.agents/notes/` 交接笔记形态（按主题分文件 +
  固定骨架 + 中英双语），§2.2 的 journal 条目骨架直接吸收它。
