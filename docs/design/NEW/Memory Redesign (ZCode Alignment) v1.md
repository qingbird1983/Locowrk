# 记忆架构重设计（ZCode 对齐）v1

> **状态：待研究。** 本文是问题实证 + 目标态设计 + 方案候选，**不是已实施设计**——除本文与
> INDEX.md 登记行外零文件改动、零代码改动，与《AUDIT-FIX-PLAN-v1》审计修复线互不占用。
> 立稿日期 2026-10-11；参照系 = ZCode 3.15.1 记忆子系统源码（`F:\Warehouse\Github\ZCode`，
> 权威设计文档 `apps/zcode-cli/docs/design/v2/memory/README.md`，v4 spec）与本仓库 v2.154 实况。
> 行号证据以当日工作区为准，实现时须重新核对（本仓库纪律：验证器自己也会骗我）。

---

## §0 动机：一次真实失败的记忆召回（本稿全部结论的出发点）

用户在 Locowrk 桌面端、项目 `F:\Warehouse\TITY` 新开会话，第一句话问「查一下你之前的记忆」。
该会话此前一天刚在同一项目做过完整工作（打砖块游戏 + 两轮自动化测试，记忆树里留着 27 KB
的真章节与轮总结）。模型的回答是：**「持久化记忆库（memory_*）是空的——这个项目的记忆全部
在工作区的 `.Locowrk/` 目录里」**，然后只凭项目文件夹里的 handoff.md / requirements.md 拼出了回忆。

四条根因，全部实证（只读核查，2026-10-11）：

| # | 根因 | 证据 |
|---|---|---|
| 1 | **事实层真的是空的**。`~/.Locowrk/memory.sqlite3` 的 `memories` 表 **0 行**；`memory/global/MEMORY.md` 只有 17 字节标题。`memory_list` / `memory_search` 查的就是它，返回空是**正确行为** | 只读 sqlite 实测；写入唯一生产路径 = 模型主动 `memory_write`（v2.136 退役 autoExtract 后），历史上从未发生过 |
| 2 | **真有货的树投影够不到**。`memory-tree/F__Warehouse_TITY-94676677/` 有 27 KB 章节数据，`memory_project_history` 本可命中；但它是 `Exposure::OnDemand`，新会话零会话内用量、embedding 又未配置（语义相似度退化），被 Top-20 工具截断**挤出目录**——模型根本看不见这个工具的存在 | `desktop/src-tauri/src/organize/project_history.rs`（exposure OnDemand，已亲核）；`crates/kernel-agent/src/middleware/tool_exposure.rs:181-227` 截断逻辑；`~/.Locowrk/settings.json` embedding 三键全 null |
| 3 | **注入面三重盲**。`<persistent_memory>` 只注全局层（0 行 → 空块）；树目录注入只含**当前会话**的 L0（新会话 → 空）；guidance 明令「never as a way to start a conversation」 | `desktop/src-tauri/src/memory.rs:437-448`（recall 写死 `WHERE workspace=''`）；`organize/assemble.rs:157-174`（plan 只装本会话目录）；`crates/kernel-agent/src/middleware/memory_inject.rs:128-137`（GUIDANCE_BLOCK，已亲核） |
| 4 | **模型的实际动线绕开了有货的库**。audit.jsonl 尾部实证：该会话调了 `memory_list`、`memory_search(query="TITY")` → 落空；`search_tools("项目历史时间线 memory_project_history")` → 找到了工具描述但未调用；随后 `fs_read .Locowrk/handoff.md` → 用文件拼回答 | `~/.Locowrk/audit.jsonl` 尾部（982 KB） |

**一句话诊断**：检索链路本身是接通的（注入 + 工具双面都在、注册无缺失）；坏在
**冷启动盲（不知道有历史）× 检索工具不可见（看得见的查不到货，查得到货的看不见）×
写入侧怠工（有货的层靠模型自觉，从未发生）** 三者叠加。这与 ZCode 的设计取舍形成
直接对照——见 §1。

---

## §1 参照系：ZCode 记忆子系统勘察结论

（源码位置统称 `core/src` = `F:\Warehouse\Github\ZCode\apps\zcode-cli\packages\core\src`；
逐条 file:line 见附录 A。）

### 1.1 存储模型：单事实单文件 + 一行指针索引

- 目录：`<cliStorageRoot>/memories/projects/<slug>-<hash16>/memory/`，其中
  `hash16 = sha256(规范化路径).hex 前 16 位`（win32 先 toLowerCase），`slug` = basename
  净化后 ≤48 字符。目录名自解释且可反解显示名。
- 每个事实一个 `.md` 文件，YAML frontmatter：`name`（kebab slug）/ `description`
  （一行，供召回判相关性）/ `metadata.type ∈ user|feedback|project|reference` /
  `metadata.originSessionId`。正文用 `[[other-memory-name]]` 互链，悬空不算错。
- `MEMORY.md` 索引：无 frontmatter，一行一条指针 `- [Title](file.md) — hook`；
  **内容永不进索引，索引永不放正文**。注入前剥 frontmatter/HTML 注释并钳
  **200 行 / 25,000 字符**，超限附 WARNING。
- `originSessionId` 由**写入边界代码**补（仅首次，已有值保留），不由模型生成。
- mtime 是唯一新鲜度信号：驱动 manifest 排序、「N days old」陈旧警告、viewer 展示；
  **不建任何索引库 / metadata 文件**（spec 明文）。

### 1.2 上下文联动：索引全文常驻前缀（压缩豁免区），正文永不自动进

- 会话初始化一次性把 `MEMORY.md` **全文**注入首条 user 消息前的 `<system-reminder>`
  附件（context_prefix 区），导语「user's auto-memory, persists across conversations」。
- 该区对 compaction **完全豁免**（prefix 判定 = system + context_prefix 附件；摘要范围
  排除 prefix）——记忆不会被压缩误伤、也不会压缩后重复注入。
- system 另有约 1.5 KB 的 `# Memory` 指令段常驻（怎么写、格式、不记什么）。
- 按需召回分支（LLM selector 从 ≤200 项 manifest 选 ≤5 个文件、单文件读入钳 200 行 /
  4096 字节、全会话召回预算 61,440 字符、48 h 以上附「N days old — verify」）**已实现但
  默认关闭**（`PROJECT_MEMORY_SEMANTIC_RECALL_ENABLED = false`）。当前实际形态 =
  纯「索引常驻 + 模型按需 Read」。

### 1.3 写入：后台提取子代理是主路径，模型自觉只是辅路

- **A. 每成功 turn 后台提取（主路径）**：turn 成功收束后 fire-and-forget 调度一个提取
  子代理（≤5 turns；继承本轮主模型但压到最低推理档 + maxOutputTokens 5000；
  `skipTranscript` 防自反馈；上下文 = 主对话 entries 快照 + 提取提示词）。调度器 =
  单 running 槽 + 单 latest-pending 槽合并去抖，cursor = 已处理消息 UUID，只有成功/无操作
  才推进。资格门：本轮模型已直写记忆 → 跳过；无非 meta 用户消息或正文 <3 词 → 跳过。
  提取提示词要点：先并行读再并行写；**注入现有 manifest 供查重——更新既有文件而非建重复**；
  没东西记就输出字面量 `Nothing to save.`；禁止去 grep 源码/git 验证。
- **B. 前台直写（辅路）**：system 指示模型直接用 Write 写记忆目录；落盘在记忆目录内的
  Write/Edit **自动放行权限**。用户说「记住 X」走这条，且该轮后台提取被 A 的资格门短路。
- **C. Dream 整理（已实现、未接线）**：24 h 门 + `.consolidate-lock` 文件锁（PID +
  1 h 新鲜期 + 活性检查）+ ≥5 个新 transcript 才跑；四阶段 orient/gather/consolidate/prune
  （≤50 turns）：合并进既有主题文件、相对日期转绝对、**删除被现实推翻的事实**、
  MEMORY.md 保持 <200 行/<25 KB。**当前产品链路不自动触发**——其直接后果就是实测中
  194 KB 的失控大文件，只能靠读侧截断兜底。
- 并发：临时文件 + rename 原子写（O_NOFOLLOW、fsync）；`expectedRevision`（mtime+size
  派生）乐观并发 + read-before-write 强制；提取单飞；无跨会话互斥锁（靠原子写 + 乐观并发兜底）。

### 1.4 作用域：主代理只有项目记忆，全局刻意不做

- 主代理记忆根**只按 workspace 键位**，没有跨项目全局记忆注入（spec §3/§11 明文）。
  用户级记忆只存在于**命名子代理**的 Persistent Agent Memory（profile 声明
  `memory: user|project|local` 三 scope）。
- taskType 门禁：`workflow_child / subagent_child / nested_workflow_child` 一律无项目记忆；
  resume 时记忆根钉死为会话落盘时的 workspaceIdentity（防 Bash cd 漂移）。
- 工具面：**零专用记忆工具**（无 memory_read/search）——读写就是普通文件工具 +
  权限放行；跨会话历史另有 `ReadSessionContext` 工具按需查。

### 1.5 控制面

`features.memory`（配置文件，默认 true）→ `memory.use`（配置，默认 true）→ 桌面全局
`memoryEnabled`（**默认 false** 的 kill switch，runtime 物化时冻结一次）；UI 设置页 +
只读 viewer（项目 → 文件 → 正文三级浏览，5 MiB 预览上限，无编辑）；headless 默认关提取，
`--memory-bench` 才开。

### 1.6 对 Locowrk 最有价值的五条

1. 「索引常驻（有硬上限）+ 正文永不自动进」是冷启动可感知的关键——**Locowrk 缺的正是这层**。
2. 写入主路径是**基础设施自动提取**（每 turn、带资格门、防自反馈）；「模型自觉写」只是辅路。
   实证：用户机器上 ZCode 已为 locowrk 项目积累 194 KB 记忆，而 Locowrk 事实库 0 行。
3. 一切内容整理决策交给**带窄工具面的后台小代理 + 冻结提示词**；运行时只管 I/O 安全、
   并发与预算截断。
4. mtime 唯一新鲜度信号，不建索引库；读侧截断 + 陈旧警告兜底写侧失控。
5. 主代理不做全局注入（用户级下沉给子代理 scope）——Locowrk 的双层设计在此**多一层**，
   保留（见 §3 的映射）。

---

## §2 设计原则（目标态的八条纪律）

1. **冷启动可感知**：新会话第一条消息之前，模型必须能看到「本项目有什么记忆、有过什么
   历史」的索引级视图。治 §0-2/§0-3。
2. **注入为主、工具为辅，注入必须进压缩豁免区**：Locowrk 的对应物 = 注入放 system
   （`Context and Memory.md` §3.6 现行口径「注入内容放 system prompt 永不压缩」），
   与 ZCode prefix 豁免区同构；注入块**会话内字节钉死**（M1 预取纪律，防打断 KV 前缀缓存）。
3. **写入不能只靠模型自觉**：必须有与既有管线同频的自动落库路径（§5 决策点）。
4. **硬预算 + 读侧截断兜底**：每个注入面都有显式上限；写侧失控由读侧钳制兜底（ZCode 的
   194 KB 教训）。
5. **updated_at/mtime 是唯一新鲜度信号**：不新增索引库、不加 metadata 文件；陈旧提示由
   注入侧按时间差现算。
6. **双层作用域保留**：全局层（用户偏好）+ 项目层是 Locowrk 优于 ZCode 的地方，不砍；
   用 ZCode 的四类型做归层映射（§3）。
7. **子代理默认无项目记忆**（两家一致）：未归属会话只见全局层、项目写拒绝——现状保留。
8. **内容决策给记录员，运行时只管机制**：提取/整理用带窄工具面的后台代理 + 冻结提示词；
   宿主负责原子写、去重、预算、截断。

---

## §3 分层模型（目标态）

| 层 | 键位 | 装什么 | 来源（ZCode 类型映射） | 注入策略 | 现状差距 |
|---|---|---|---|---|---|
| **L-G 全局层** | `memory.sqlite3` `workspace=''` | 用户偏好、协作纠正（Why/How to apply） | `user` / `feedback` | 自动注入 top-5（现状保留，单条钳 500 字） | 机制在、数据空（§5 治） |
| **L-P 项目层** | `memory.sqlite3` `(workspace, key)` | 项目约束、决策、进展事实、外部资源指针 | `project` / `reference` | **索引注入（新增，§4.1）** + 工具按需 | 索引从不进上下文（§0-3） |
| **L-T 会话树层** | `memory-tree/<scope>/<session>.json` | 章节捆 L0/L1/L2、轮总结——「做过什么」的真源 | （ZCode 无对应物，Locowrk 独有优势） | 本会话 L0 目录（现状）+ **常驻进展行（新增，§4.2）** + 树三件工具 | 跨会话工具 OnDemand 被挤出（§0-2） |
| **L-R repo 交接层** | `<repo>/.Locowrk/` | 人读交接文件（requirements/handoff/notes） | （提醒制，非数据库） | 静态提醒段指路（现状不动） | 无 |

归层规则：记录员/提取产出**必须带类型标注**，`user|feedback → L-G`，`project|reference →
L-P`；树整理（L-T）不改记忆层，事实卡是整理的**副产物**（§5 提案 A）。

**真源声明**：L-G/L-P 的结构化真源 = `memory.sqlite3`（查询、去重、undo 都靠它）；
`memory/projects/<project>/facts/*.md + MEMORY.md` 是**人读投影**（memlayout 双写已有）。
注入索引优先读 md 投影（人可编辑、与 ZCode 同构），md 缺失/为空时由 sqlite 现合成只读
索引（不回写文件）。**不采用 ZCode 的「纯文件、无专用工具」路线**：Locowrk 的 memory_*
插件工具免费提供 scoping、undo（补偿契约）、audit 三样，ZCode 为此自己造了
origin-session 盖章 + 权限放行 + 无 undo——保留现役工具面，判为**有意分歧**，不追。

---

## §4 注入面设计（治「查不到」的直接修复 = 未来批 M1，纯读侧）

### 4.1 项目记忆索引注入（新增，对照 ZCode 1.2）

- **内容**：本项目 L-P 层的索引行——md 投影的 `MEMORY.md` 指针行（md 缺失则由 sqlite
  `WHERE workspace=?` 按 `updated_at DESC` 合成，每行 `- <key> — <tags>（N 天前）`，
  「N 天前」由注入侧按 `updated_at` 现算、>48 h 才标，不写进文件）。
- **钳制**：200 行 / 25,000 字符（与 ZCode 同数），超限截断 + 一行 WARNING（指向
  `memory_list` / `memory_search`）。
- **位置与钉死**：并入 `MemoryInject` 预取块（现有 `(session_id, scope_id)` 预取缓存
  自然保证会话内字节钉死），作为 `<persistent_memory>` 之后的独立小节
  `<project_memory_index>`；归属失败/项目层空 → 整节缺席（零开销）。
- 与 v2.136「项目层事实不自动注入」裁定的关系：**不冲突**——注入的是指针索引不是事实
  正文，正文仍按需 `memory_read`；该裁定禁的是「项目事实全文灌进 system」。

### 4.2 常驻进展行（解冻《Cross-Session Recall Design v1》批 2）

该稿批 2「常驻进展行注入」当时被裁定「暂缓——实测确有需要再立批」（其 §6/§77/§94 三处
挂账）。§0 的实测就是这个凭证，**本稿正式解冻批 2**，设计如下：

- **内容**：`TreeStore::scan_workspace(workspace)` 聚合——已整理会话数、最近整理时间、
  最近 1-2 份树的最新若干章节标题（L0 级，10 token 级别）+ 指路一句
  （「历史详情用 memory_project_history / memory_search_titles，章节下钻用 memory_expand」）。
- **钳制**：≤400 字符。
- **位置与钉死**：随树目录段进 system 殿后位（`TreeContextSource::plan` 增进展字段或
  并入 directory 文本前段）；**首请求算毕即钉死**（新增会话级 pin 缓存，仿
  `memory_inject.rs` 的 `PREFETCH_CAP` 纪律）——中途 organize 产新章不得改写本会话
  已注入字节（防前缀缓存失效；ZCode 的 memory_update 通知机制证明逐次刷新是歧路）。
- 空 scope（全新项目）→ 不注入任何块。

### 4.3 曝光修正

- `memory_project_history`：`OnDemand → Always`（`organize/project_history.rs` spec 处）。
  理由：它是「用户问过去」动线的入口工具，本地 JSON 聚合零网络零模型成本，Always 名额
  消耗（约 14 → 15 个 Always，Top-20 下 OnDemand 名额 6 → 5）可接受；`max_tools` 是
  设置项，紧张时可由用户调。
- `memory_search_titles` 维持 OnDemand（带 query 才有意义；`search_tools` 可找回）。
- `memory_collapse` 维持 Hidden + 树门（现状）。

### 4.4 guidance 订正（`memory_inject.rs` GUIDANCE_BLOCK，静态文本随批改）

在现有文本上增两句、开一个显式口子：

1. 「Empty results from memory_read / memory_search do **not** mean there is no history:
   project history lives in the memory tree — call memory_project_history first, and
   memory_search_titles for topics.」
2. 把「never as a way to start a conversation」限定为「don't probe memory unprompted;
   **when the user explicitly asks about past work or memories, always query the tools
   above rather than assume from injected context**」。

### 4.5 批 M1 验收判据（立此存照）

- 单测：索引注入（md 有/无/sqlite 兜底/钳制/字节钉死）；进展行（有树/空树/钳制/会话内
  二次请求字节不变）；exposure（Always 不被截）。
- 端到端走查：desktop 真跑——TITY 新会话首问「查一下之前的记忆」，验收 = 回答包含
  `memory_project_history` 返回的时间线内容（打砖块批次），而非只有 `.Locowrk/` 文件。
- 四道闸 + fmt/clippy 双侧照常。

---

## §5 写入面设计（核心决策点 = 未来批 M2，**需用户批准 v2.136 裁定的部分反转**）

事实：v2.136 退役 autoExtract 后，事实层写入唯一路径 = 模型主动 `memory_write`；
§0-1 实证该路径在实际使用中**一次都没发生过**（0 行）。ZCode 的答案（§1.3）是基础设施
自动提取。三个候选：

### 提案 A（推荐）：整理副产物落库——记录员顺带产出事实卡

- **时机**：复用现役整理管道（`organizer.after_turn` → 阈值/增量触发 → 反弹熔断 →
  异步记录员 → 校验失败转规则切分）。整理收束时，记录员在章节 + RunSummary 之外追加
  产出 **≤N 条事实卡**（建议 N=3，设置可调）：每张卡 = key 建议 + value（≤500 字）+
  `type ∈ user|feedback|project|reference` + tags。
- **归层**：按 §3 映射 upsert 进 sqlite（`user|feedback → workspace=''`，其余 → 当前
  workspace），md 双写沿用 memlayout。**去重靠 upsert + 提示词**：记录员提示词注入
  现有 key 清单（ZCode manifest 注入同款），要求「更新既有 key 而非造近重复」。
- **降级语义**：规则切分路径产不出事实卡就产不出（诚实缺省，不硬造）；反弹熔断同时
  管住事实卡成本（与整理同频，不是每轮）。
- **成本**：零新增常驻管线、零新增调度器——触发/重试/降级/熔断全部白得。
- **与 v2.136 的关系**：该裁定退役的是「从对话自动提取事实」的**每轮**路径；本提案是
  「树整理」的登记副产物，频率 = 整理频率，且**需要用户对本条作出新的批准**（裁定变更
  留痕，写进 CHANGELOG 批说明）。
- **设置面**（可配置原则）：`memory.factCards`（缺省值随批准结论定）、
  `memory.factCardMax`（缺省 3）。

### 提案 B：ZCode 式每轮后台提取子代理

每成功 turn 调度独立提取代理（§1.3-A 全套：单飞 + 光标 + 资格门 + skipTranscript）。
更及时，但要新造一条常驻后台管线（调度器、光标、快照、drain），每轮 token 成本，
且与 v2.136 裁定冲突面最大。**仅当提案 A 实测粒度不够密时再议**。

### 提案 C：不自动写，只做批 M1

接受事实层长期为空、记忆检索主要靠 L-T 树层 + L-P 索引（索引也会一直空）。
成本最低，但「记忆点查询」永远缺一只角——与用户「这块是最大的问题」的定性不符。

> **本稿推荐 A**。批 M2 立项前需用户在 A/B/C（或 A 的参数）上拍板；未拍板前 M2 不立项。

---

## §6 预算与生命周期

| 项 | 上限 | 说明 |
|---|---|---|
| 项目索引注入 | 200 行 / 25,000 字符 | 注入侧钳制 + WARNING（§4.1） |
| 单条事实注入（L-G top-5） | 500 字/条 | 现状保留 |
| 进展行 | 400 字符 | §4.2 |
| 单 value 落库 | 256 KiB | 现状（memory-sqlite）保留 |
| 事实卡 | ≤3 张/次整理（可调） | §5 提案 A |
| 陈旧提示 | >48 h 标「N 天前」 | 注入侧现算，不改文件 |

- **去重/演化**：upsert by key + 记录员提示词查重（ZCode 同款：运行时不做程序化 merge，
  靠提示词约束 + upsert 幂等）；**错误记忆靠模型/管理页删除**（memory_delete 现状）。
- **深度整理（Dream 对应物）**：挂账不做——ZCode 自己也未接线，先靠 upsert + 读侧截断
  兜底；若未来项目层 facts 膨胀失控再立专项。
- **键位归一化（趁早！）**：`memories` 表键 = workspace 原样字符串，Windows 大小写/
  斜杠漂移会造成同项目双键位（探索已列为断点）。**当前 0 行，归一化零成本**——批 M1
  或 M2 顺带做：写入/查询边界统一 `resolve + win32 小写`（ZCode §1.1 同款）。树 scope
  的 fnv1a 同病但已有 TITY/Timo 两份数据，树是投影可重建（树 v2 §1.3），归一化 + 重建
  说明放批 M3。

## §7 明确不做（边界记档，两家一致或已有裁定）

1. **跨项目检索**：设计已否决（会改写他会话 system 前缀、打断缓存）；ZCode 主代理同样
   只有项目层。全局时间线不做。
2. **向量/embedding 作为检索主路径**：ZCode 的 selector 分支（无向量、纯 LLM 挑选）默认
   关；Locowrk 保留关键词降级 + 可选 embedding 配置（用户在设置页自配），不立为主路径。
3. **`/memory` 斜杠命令**：ZCode 也没有；管理走桌面管理页（现状）。
4. **MEMORY.md 程序化重建**：两家都是模型维护；运行时只在注入时清洗 + 钳制。
5. **子代理项目记忆**：维持现状（未归属 → 只见全局层、项目写拒绝）；ZCode 的
   taskType 门禁是同一边界。
6. **ZCode 式「文件直写 + 权限自动放行」**：不采纳（§3 有意分歧条目）。

## §8 批次路线（审计主线收口后立项；本稿不排期、不占审计线）

| 批 | 内容 | 规模 | 前置 |
|---|---|---|---|
| **M1（读侧）** | §4 全部：索引注入 + 进展行（解冻批 2）+ exposure 修正 + guidance 订正 + workspace 键位归一化（0 行窗口期）+ §4.5 验收 | 小（1-2 天面） | 无技术前置；等审计批间隙 |
| **M2（写侧）** | §5 按用户拍板的提案落地 + 设置面 + 提示词 + 测试 | 中 | 用户批准 §5 提案 + v2.136 部分反转 |
| **M3（清债，可选）** | 遗迹清理（`memory/memory.sqlite3` 0 表遗物、`experiences.sqlite3` 退役空库）+ 树 scope 键位归一化 + 重建 + embedding 配置引导 | 小 | 涉用户数据目录，需单独确认 |

每批照常：先勾 ROADMAP → CHANGELOG 落点 → 四道闸 + fmt/clippy 双侧 → desktop 端到端走查。

---

## 附录 A：ZCode 勘察证据表（仓库根 = `F:\Warehouse\Github\ZCode`）

| 结论 | 证据 |
|---|---|
| 项目记忆根键位算法 | `apps/zcode-cli/packages/core/src/memory/project-root.ts:10-33` |
| MEMORY.md 钳制 200 行/25,000 字符 + 清洗 | `core/src/memory/index-content.ts:3-4,8-11,20-38,49-67` |
| frontmatter 格式与四类型 | `core/src/context/sections/memory.ts:38-51`；`core/src/memory/recall/types.ts:3` |
| originSessionId 写入边界补 | `core/src/memory/origin-session.ts:8-51`（接线 `tool/handlers/write.ts:118-123`、`edit.ts:501`） |
| 索引注入 context_prefix + 导语 | `core/src/context/sections/request-user-context.ts:63-88`；`core/src/runtime/methods/context.ts:182-211` |
| prefix 压缩豁免 | `core/src/runtime/helpers/compact-selection.ts:141-145,209-227,337-349` |
| 语义召回分支默认关 | `core/src/memory/project-memory-retrieval-branch.ts:4` |
| 召回预算/单文件钳制/陈旧警告 | `core/src/memory/recall/index.ts:20`；`recall/content.ts:5-7,46-60,73-99`；`recall/manifest.ts:12-38,120-124`；`recall/selector.ts:15-38,116-141` |
| 每 turn 触发提取 | `core/src/runtime/methods/turn.ts:749-754` |
| 提取资格门（直写短路 / ≥3 词） | `core/src/memory/extraction.ts:6,68-83,228-293` |
| 提取单飞 + 光标 | `core/src/memory/extraction.ts:85-179` |
| 提取子代理（≤5 turns / 最低推理档 / skipTranscript / 窄工具面 / 两轮策略） | `core/src/runtime/helpers/project-memory-extraction.ts:19,45-80`；`core/src/model/auxiliary-model-options.ts:3-14`；`core/src/runtime/helpers/project-memory-agent.ts:36-96`；`core/src/memory/memory-agent-loop.ts:41-165,275-287`；`core/src/memory/extraction.ts:42-66`（提示词） |
| 前台直写权限放行 | `core/src/tool/executor/memory-file-permission.ts:19-86` |
| Dream 门与四阶段、未接线 | `core/src/memory/dream.ts:9-12,40-93,95-170`；`core/src/runtime/helpers/project-memory-dream.ts:28-29,54,124-136` |
| 原子写 + 乐观并发 + read-before-write | `apps/zcode-cli/packages/adapters/src/fs/index.ts:297-306,473-483,700-749`；`core/src/tool/handlers/write.ts:105,133,278-304` |
| taskType 门禁 | `core/src/runtime/helpers/project-memory.ts:4-27` |
| 用户级记忆（子代理 scope） | `core/src/subagent/persistent-memory.ts:10-31,96-100`；`persistent-memory-prompt.ts` |
| 三层开关 | `packages/contracts/src/config/index.ts:36,41`；`packages/adapters/src/config/index.ts:291-293`；`packages/services/src/node.ts:2190`；`packages/bootstrap/src/zcode-protocol/server-operations.ts:3390-3392` |
| 只读 viewer / 5 MiB | `packages/services/src/memory/memoryService.ts:33-35,88-124,141-151`；`projectMemoryStableRead.ts:8` |
| resume 钉死记忆根 | `core/src/runtime/methods/resume.ts:126-133` |
| ReadSessionContext 工具 | `core/src/tool/handlers/read-session-context.ts:163-176` |

## 附录 B：Locowrk 现状锚点（仓库根 = 本仓库）

| 事项 | 锚点 |
|---|---|
| 数据根解析（`~/.Locowrk`） | `crates/plugin-api/src/fsutil.rs:21-31`；`desktop/src-tauri/src/data_root.rs:12-20`；`state/boot.rs:66` |
| 事实库 schema（(workspace,key) 双层） | `plugins-src/memory-sqlite/src/store.rs:25-56`；`desktop/src-tauri/src/memory.rs:152-173,473-514` |
| md 双写/目录布局（L-G/L-P 投影） | `crates/plugin-api/src/memlayout/mod.rs:9-17,45-47,129-139`；插件侧 `store.rs:488-533` |
| memory_* 工具面与曝光 | `plugins-src/memory-sqlite/src/lib.rs:64-199`（write/read/search=Always，list/delete/undo=OnDemand，:94 注三件套不受挤出） |
| 树三件工具与曝光 | `desktop/src-tauri/src/organize/tree_tools.rs:44,64`（expand=Always）；`search_titles.rs:43,60`；`project_history.rs`（OnDemand，已亲核）；`memory_collapse` Hidden+树门 `tool_source.rs:35-40` |
| Top-N 截断 / EWMA / 钉死 | `crates/kernel-agent/src/middleware/tool_exposure.rs:64-72,181-227,301-336`；EWMA 半衰期 `:44-49`；usage 源 `state/boot.rs`（usage.sqlite3） |
| 注入链注册序 | `desktop/src-tauri/src/state/waterfall.rs:60-115`（MemoryInject :88-94；TreeDirectoryInject :112 殿后） |
| 全局层 recall 写死 `WHERE workspace=''` | `desktop/src-tauri/src/memory.rs:437-448,526-544`；top_k/500 字 `:117` |
| guidance 静态文本 | `crates/kernel-agent/src/middleware/memory_inject.rs:120-137`（已亲核）；预取缓存 `:30-36,268-290` |
| 树目录只注本会话 | `desktop/src-tauri/src/organize/assemble.rs:65-80,157-174`（已亲核）；契约 `crates/kernel-agent/src/kernel/services.rs:144-147` |
| 树投影存储/scope 编码 | `crates/memory-tree/src/storage.rs:17,31-49,73-238,190-202` |
| 会话归属旁车/解析链 | `desktop/src-tauri/src/workspace.rs:3,14-15,47-68`；`state/memory_stack.rs:54-56`；归属点 `commands.rs:54-66`、`commands/sessions.rs:46-60` |
| 整理管道（触发/记录员/降级/熔断/重投） | `desktop/src-tauri/src/organize/mod.rs:234-337,496-531`；`commands.rs:71-79,118`；`state/background.rs:132`；提示词契约 `crates/memory-tree/src/manager.rs:29-70` |
| v2.136 退役三键 | `desktop/src-tauri/src/settings/memory.rs:59-64` |
| repo 交接层 | `desktop/src-tauri/src/repo_context.rs:17,30-43,69-80` |
| 实测数据（2026-10-11 只读） | `memories` 0 行；`memory/global/MEMORY.md` 17 字节；`memory-tree/` 仅 TITY(27KB)+Timo 两 scope；`experiences.sqlite3` 0 行；`memory/memory.sqlite3` 0 表遗物；embedding 三键 null；audit.jsonl 尾部检索动线（§0-4） |
| 相关现行规范 | `docs/design/Context and Memory.md`（§3.6 注入顺序 / §4.6 双层隔离）；`docs/design/Memory Tree Design v2.md`（v2.0 权威）；`docs/design/Cross-Session Recall Design v1.md`（批 1 已兑现、批 2 挂账 ：6,:77,:94）；`docs/design/Tool exposure strategy and usage statistics.md` |
