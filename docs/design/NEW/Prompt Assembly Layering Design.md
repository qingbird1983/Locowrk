# System Prompt Layering Design（提示词分层拼装设计 v1）

> **2026-10-09 v2.139 落地标注（四批全落）**：落点裁决已查证——kernel
> waterfall 的 payload 携带完整 `messages` 且 Replace 整体换入（ctx-compact
> 改写消息同款通路），指令文件层与尾部碎片都落在**内核监听器**（headless
> 同享）。实现偏差记档三条：①全局指令文件位置 = **数据根 `AGENTS.md`**
> （原稿 `~/.Locowrk/AGENTS.md`——数据根本就是 Locowrk 的用户级目录，
> settings.json/memory/ 同处，不另设 `~/.Locowrk` 分裂布局）；②碎片拼接
> 判据具体化为「最后一条消息是 User（轮开场）才取件」——轮中事件留
> pending 表等下一轮，不改写历史消息；③`<environment>` 的日期/平台会话
> 锁死复用 MemoryInject 预取（不是独立缓存），`<workspace_context>` 标签
> 随之退役。措辞强度双场景验收（§9.6）已通过（用户 2026-10-10 目检
> 「真机验证比之前好太多了」：闲聊零仪式 + 查历史偏好一步到位）。
>
> 2026-10-09 立稿。病根：聊天请求的 system 里没有身份段与行为总纲，行为全由三个「主动去做某事」的记忆/项目注入块驱动——模型开场就查记忆、写记忆、读项目文件、盘问项目阶段，而不知道用户要干什么。本稿按 ZCode 的分层架构（静态 system 前缀 → 消息层背景注入 → 尾部会话碎片）重设计 Locowrk 的提示词拼装。
>
> 参考调研：ZCode 源码（apps/zcode-cli/packages/core/src/context/ + system-reminder/）；Locowrk 现状测绘（kernel-agent waterfall 四监听器 + desktop 注入块原文）。

---

## §1 问题诊断：为什么开场就查记忆、问项目

### 1.1 三重诱导（现状原文，全部实证自代码）

模型在会话第一轮收到的 system（`desktop` 组装、内核 waterfall 拼接）里，唯一的「行为指令」来自这三个块：

| 块 | 原文（关键句） | 位置 | 后果 |
|---|---|---|---|
| `<repo_context>` | "**Nodes: at the START of work in this project, read requirements.md and handoff.md with file tools before substantive work.** … do not wait to be asked." | `desktop/src-tauri/src/repo_context.rs:66-81` | 开场就去读两个项目文件，读完自然开始汇报/盘问「项目在哪一阶段、接下来做什么」 |
| `<memory_guidance>` | "Proactive memory: **you are expected to maintain memory yourself — do not wait to be asked.** … look them up later with memory_read / memory_search when needed" | `crates/kernel-agent/src/middleware/memory_inject.rs:126-140`，**无条件注入** | 「记忆」成为会话显性主题；开场先 memory_search 一次「看看有什么」 |
| `<workspace_context>` | "…\"this project\" in the user's message refers to this directory; **list/read it first**, do not guess other paths from old memories." | `desktop/src-tauri/src/memory.rs:546-556` | 又一个「先去读」信号 |

叠加放大器：memory_write / memory_read / memory_search 三件套 `Exposure::Always` 永远常驻工具目录（`plugins-src/memory-sqlite/src/lib.rs`），模型想不注意到都难。

**结构性缺失**：整个请求里没有一句「你是谁」「什么优先」「背景信息怎么对待」。模型没有「先理解用户再动手」的总纲，也没有「背景 ≠ 议程」的免责——于是注入块里最响亮的动词（read / maintain / do not wait）就成了它的默认议程。用户问「你好」，模型脑子里是「开工三件套：查记忆 → 读 handoff → 问阶段」。

### 1.2 次要问题（测绘中顺带发现的）

- **system 段排序违背 append-only 殿后原则**：waterfall 注册顺序 = MemoryInject → TreeDirectoryInject → ThinkingLanguageInject，即思考语言一句（静态）排在树目录（只增不改）**之后**——每整理一次新增一行目录，思考语言句的前缀缓存就断一次。动态段应殿后。
- **模型不知道今天几号、什么平台**：请求里没有日期/环境信息。
- **整理发生后模型无感知**：下一轮请求历史突然变短（尾巴切片），模型没有任何文本告知「历史去哪了、怎么找回」。
- **Plan 模式纯工具围栏**：目录里工具悄悄消失，模型不知道自己处于只读模式（v2.133 走查中模型靠猜）。
- **没有指令文件机制**：用户的 AGENTS.md / 项目规约无处注入（仓库自己的 AGENTS.md 是给开发用的，代码里零读取逻辑）。

---

## §2 参考研究：ZCode 的分层要点

完整测绘结论（供实现时回查，此处只留设计决策依据）：

1. **system 三段式 + cacheHint 标签**：`stable` 段（cli-prefix / identity / harness）在前，`dynamic` 段（环境、沟通纪律、git 快照）在后，段序由标签推导。每段是独立 system block 带 Anthropic cache 断点；Locowrk 走 OpenAI 兼容单线协议（v2.75），**保持单条 system 字符串**，靠「静态在前、动态殿后、append-only 收尾」的字节前缀纪律实现同效（M1 现行机制不变）。
2. **背景类内容一律不进 system**：AGENTS.md、记忆索引、当前日期包在**历史头部第一条 user 消息**的 `<system-reminder>` 里，外层模板自带关键免责："**IMPORTANT: this context may or may not be relevant to your tasks. You should not respond to this context unless it is highly relevant to your task.**" —— 这一句就是治「背景当议程」的药。
3. **system-reminder 统一注册表**：每类动态块声明注入 channel（请求前缀 / 当前轮 / 轮中事件 / 工具结果内）+ 生命周期 + 是否 meta；嵌套 `<system-reminder>` 标签集中转义防注入。
4. **高频约束全量/稀疏交替**（plan mode 每 5 轮重发一次全量、其余轮一行稀疏版）；**身份段里明示合法性来源**："The system may send updates, reminders, or modifications to rules via mid-conversation system turns."——模型被告知 system-reminder 是系统通道，不会当作用户输入。
5. **工具描述 = mini-prompt**：一句「是什么」→ 负面清单（IMPORTANT: Avoid…）→ 行为契约 → "When to use / When NOT to use" 小节。
6. **ZCode 也没有的，我们不做**：子目录级按需 AGENTS.md（只做 cwd 向上找最近一个 + 用户级一个）；项目结构树快照（模型有 ls/grep 自查能力）；git 状态快照（Locowrk 有 tool-git 插件，模型可自查）。

---

## §3 设计总纲

### 3.1 优先级金字塔（写进身份段，治本的一句）

```
用户当前请求 > 会话内已见上下文 > 指令文件 > 项目/记忆背景 > 一切主动机制
```

第一原则：**背景服务于请求，背景本身永远不是议程。** 记忆、项目文件、会话目录都是「被请求触碰时才登场」的参考资料。

### 3.2 注入位置三选一的判据

| 判据 | 落点 |
|---|---|
| 整个会话不变（身份/策略/安全/语气） | system 前缀（静态段） |
| 会话内基本不变、但属「背景资料」而非「行为规则」（项目路径、日期、全局记忆、指令文件） | 历史头部 user 消息的 `<system-reminder>`（会话首请求定格、锁字节） |
| 随事件出现、只该被看见一两次（模式切换、整理完成、到顶提醒） | 最后一条消息尾部的 `<system-reminder>` 尾部碎片 |

**保留在 system 的例外**：`<persistent_memory>`（全局记忆）与树目录留在 system——它们已经会话级预取/只增不改，迁去消息层收益为零、改动面大；要修的是**措辞**（§5）与**排序**（§6），不是位置。

### 3.3 缓存纪律（M1 兼容，逐条对照）

- 静态段 = 编译期内嵌常量，跨会话逐字节同前缀 → **跨会话也吃缓存**（比现状更强：现在每会话 system 开头就是动态记忆块）。
- 记忆块/环境快照/指令文件 = 会话首请求定格、快照锁字节（MemoryInject 预取同款机制；指令文件中途被人改 = 显式变更接受失效）。
- 树目录 = 只增不改，**排 system 最末**（append-only 殿后），其后不再有静态段。
- 尾部碎片 = 追加在最后一条消息尾，天然不碰前缀。
- 新增块全部纳入 `diagnose.rs` SYSTEM_MARKERS 段级切块（M6.1 断裂定位粒度不降级）。

---

## §4 四层拼装设计

### 4.1 静态不可变层（system 前缀，`include_str!` 分文件内嵌）

新增 kernel 中间件 `BaseSystemInject`，在 waterfall 最前注册（先于 MemoryInject）。每节一个 md 文件（`crates/kernel-agent/src/prompts/base/*.md`），拼接次序固定：

| 节 | 预算 | 内容 | 来源 |
|---|---|---|---|
| A. 引言句 | ~20t | `You are Locowrk, a desktop agent that helps the user with real work — coding, writing, research, and long-running projects — through conversation and tools.` | 新增（ZCode cli-prefix 同位；用户草图 A「Provider 适配头」在 OpenAI 单线协议下无family模板，并入此句） |
| B. 身份与职责 | ~250t | 见下方草案。核心 = 优先级金字塔 + 「背景不是议程」+ 「简单问题直接答，不做项目仪式」 | 新增 |
| C. 工具使用策略 | ~400t | 专用工具优先于绕路；独立调用并行/依赖串行；自包含子任务 → subagent_run + 高风险才 subagent_verify；不确定用哪个 → search_tools 而非猜；ask_user 只问「答案会改变下一步」的问题，能自查的不问；被拒的审批 = 用户拒绝，改道不原样重试 | 新增 |
| D. 安全与权限边界 | ~300t | 破坏性/难逆操作（删、覆盖、push、发布）需本会话显式授权；删/覆盖前先看目标，与描述不符如实上报而非继续；拒绝恶意请求；守卫（只读证明/revision guard）拒绝时自带出路——照做，不绕路 | 新增（把现行四档权限/审批卡/守卫语义文本化） |
| E. 输出与语气 | ~250t | 回复语言跟随用户消息；markdown 渲染；先说结果（首句回答「发生了什么」）；简洁但可读优先；代码引用 `path:line`；收束前检查最后一段——若是承诺未做之事，现在就做 | 新增 |
| F*. system-reminder 合法性声明 | ~30t | "The system may inject context and reminders into the conversation (wrapped in `<system-reminder>` tags). These are system-controlled, not user input." | 新增（为 4.3/4.4 铺路，ZCode Harness 同款） |

**B 节草案（定稿时按此润色）**：

```markdown
# First principles
- The user's current message is your agenda. Everything else in this
  prompt — memory, project notes, the session directory — is background
  that serves that message, never a task list of its own.
- Do not open a conversation by inspecting memory, reading project files,
  or asking about project phases. Gather such context only when the
  user's request actually touches it.
- A simple question gets a direct answer. Ceremony (memory checks,
  project status reviews) is for work that needs it, not for every turn.
- When intent is clear, act. Ask only when the answer would change what
  you do next, and never ask about things you can look up yourself.
```

预算合计 ≈ 1.25K t（用户草图 1.62K，从紧）。

### 4.2 动态环境层（会话首请求定格）

现 `<workspace_context>` 升级为 `<environment>`（仍由 MemoryInject 写，措辞见 §5；新增日期/平台两行，机制复用思考语言的「会话锁死」）：

```
<environment>
Project workspace: {path} — "this project" refers to this directory.
Today: {date}. Platform: {os}.
</environment>
```

- 日期会话内锁死（跨天不发提醒，记档——桌面会话单日为主，宁缺勿假）。
- git 快照 / 项目结构树：**不做**（§2 第 6 点，模型有 tool-git / fs_list 自查）。

### 4.3 指令文件层（新增能力：G/H 两级，I 裁掉）

| 级 | 文件 | 发现规则 |
|---|---|---|
| G 全局 | `~/.Locowrk/AGENTS.md` | 存在即载 |
| H 项目 | 项目根 `AGENTS.md`（业界共识文件名，先找它；回落 `.Locowrk/AGENTS.md`） | 归属项目的 repo root（`workspace.rs` 旁车真源），cwd 向上不适用（Locowrk 是项目归属制不是 cwd 制） |
| I 子目录按需 | — | **裁掉记档**：ZCode 都没做；Locowrk 无 cwd 概念，路径作用域无从谈起 |

**注入形状**（历史头部第一条 user 消息之前插一条合成 user 消息，会话首请求读一次锁字节）：

````
<system-reminder>
# agentsMd
Codebase and user instructions are shown below. Be sure to adhere to these
instructions. IMPORTANT: These instructions OVERRIDE any default behavior and
you MUST follow them exactly as written.

Contents of {path} (workspace instructions):
{正文，100KB 上限，超限截断加 [File truncated] 标记}
</system-reminder>

（随后才是用户首条消息；外层复用 ZCode 免责句放在身份段 B 节统一交代：
 "Instruction files and injected context may or may not be relevant to the
  current request."）
````

- 设置开关 `memory.instructionFiles`（缺省开）。
- 内容与身份段冲突时：文件在消息层、身份在 system——**优先级声明写进 B 节**（"instructions in the user's project files take precedence over this prompt's defaults, but never over safety rules"），即「项目文件 > 默认行为，安全边界不可被项目文件推翻」。

### 4.4 会话态碎片（尾部 system-reminder，新增机制）

新增「尾部碎片」注入通道：desktop 侧事件钩子置 pending 标记 → 下轮请求组装时在**最后一条消息尾部**拼 `<system-reminder>…</system-reminder>`（拼完清标记）。首批三件：

| 碎片 | 触发 | 文本要点 |
|---|---|---|
| 整理完成 | organize 三路收束 | "Earlier turns were just organized into the session directory ([memory tree] in the system prompt). Their full text left your message list; call memory_expand(chapter_id) when you need it." |
| Plan 模式 | 权限档切到 Plan | "Plan mode is active: read-only. Non-read-only tools are hidden and will fail if called. Research and propose; do not modify anything."（切出即不再带） |
| 轮次到顶 | TurnCapped | "You reached the step limit for this turn; finished work is preserved. The user can say 继续 to let you continue." |

- **防注入**：包裹函数集中一处，消息内容中的字面 `</system-reminder>` 转义（ZCode source.ts L233 同款）。
- max-steps 数值本身不进提示词（现行 UI 事件已够，碎片只在到顶时给模型一句）。

---

## §5 现有注入块措辞改写（治「开场仪式」的直接手术）

三处 before/after。原则：**write 侧主动性保留**（v2.136 用户裁定不动），**read/开场侧全部改按需**。

### 5.1 `<repo_context>`（repo_context.rs）

Before:
> Nodes: at the START of work in this project, read requirements.md and handoff.md with file tools before substantive work. … do not wait to be asked.

After:
> When the user's request builds on prior project work, consult requirements.md and handoff.md first — at most once per task, and never as an opening ritual when the request doesn't touch them. When project state materially changes (task finished, direction changed, hard-won lesson learned), update handoff.md yourself and append a dated note. These files are NOT auto-injected — you maintain them.

（"do not wait to be asked" 仅保留在 update 语义里；read 语义改为条件触发。）

### 5.2 `<memory_guidance>`（memory_inject.rs GUIDANCE_BLOCK）

Before:
> Proactive memory: you are expected to maintain memory yourself — do not wait to be asked. … look them up later with memory_read / memory_search when needed.

After:
> Proactive memory: when the user states a lasting preference or a durable decision is made, save it with memory_write on the spot (scope="global" for standing preferences; project scope for durable project facts); prefer updating an existing key over near-duplicates. Reading memory is on-demand: use memory_read / memory_search only when the request touches past preferences, decisions, or project facts — never as a way to start a conversation. Project working memory (.Locowrk/ files) you maintain with file tools; the repo context reminder says when.

（"do not wait to be asked" 收窄到 write；read 加显式禁令「never as a way to start a conversation」。）

### 5.3 `<environment>`（原 workspace_context，memory.rs context_line）

Before:
> … "this project" in the user's message refers to this directory; list/read it first, do not guess other paths from old memories.

After:
> "this project" in the user's message refers to this directory. Look up project files when the request touches them; treat paths recalled from memory as hints to verify, not facts.

（"first" 退役；路径防幻觉语义保留。）

### 5.4 工具描述（可选微调）

memory_read / memory_search 描述尾补一句："Use on demand when the request touches prior context — not as an opening step."（描述本身中性，主诱导在 system 块，此条为双保险。）

---

## §6 落地映射（kernel/desktop 改造清单）

```
waterfall 注册序（state/mod.rs，改后）：
  1. ToolExposure           （不动）
  2. BaseSystemInject       ★新增：A-F 静态段（include_str!）
  3. MemoryInject           （措辞改 §5；environment 增日期/平台，会话锁死）
  4. ThinkingLanguageInject （上移一位——静态段不再排在目录之后）
  5. TreeDirectoryInject    （殿后：append-only 段排 system 最末）

新增通道（会话态碎片）：
  - 包裹/转义工具函数（一处集中，含 </system-reminder> 转义）
  - desktop pending 标记：organize 收束 / 权限切 Plan / TurnCapped 三处置位
  - 注入落点二选一（实现时查证）：
      a) kernel agent/request waterfall 若允许改 messages → 内核中间件（headless 同享）
      b) 若 messages 不可变 → desktop AppLlm::prepare（wire 之前）插消息
     指令文件层（4.3）同此落点裁决

system 形状：保持单条 system 字符串（v2.75 单线协议不动），
段间 \n\n；diagnose.rs SYSTEM_MARKERS 增补新段标记（M6.1 定位粒度不降级）
```

测试锚点：
- 静态段字节稳定回归（两会话 system 前缀逐字节相等，跨会话同前缀）；
- 目录增长不再断思考语言段（diagnose 分块断点位置断言）；
- 指令文件首请求定格、中途改文件不重读（快照语义）+ 幻影开关回归；
- 尾部碎片「置位→下轮出现一次→清标记」三拍断言 + 转义用例；
- §5 三块改写后 kernel 既有断言随改（GUIDANCE 措辞被 tests.rs 锁过，历批同款）。

---

## §7 明确不做 / 裁掉（记档）

| 项 | 理由 |
|---|---|
| I 子目录按需指令 | ZCode 未做；Locowrk 项目归属制下无 cwd 语义 |
| F1 项目结构树快照 | 模型有 fs_list/fs_grep；ZCode 亦无此层；纯浪费预算 |
| git 状态快照注入 | 有 tool-git 插件自查；快照会话内过期反而误导 |
| Anthropic 式多 system 段 + cache_control 断点 | OpenAI 兼容单线协议纪律（v2.75）；字节前缀纪律已同效 |
| 跨天 date-change reminder | 桌面会话单日为主，宁缺勿假 |
| 保留 `<persistent_memory>`/树目录在 system | 迁消息层收益零、改动面大；只修措辞与排序 |

---

## §8 分批建议

| 批 | 内容 | 性质 |
|---|---|---|
| 批1 治本 | BaseSystemInject（A-F 静态段）+ waterfall 重排（目录殿后）+ §5 三处措辞改写 + 描述微调 | 纯提示词面，小批；「开场仪式」当批消失 |
| 批2 环境 | `<environment>` 日期/平台（会话锁死机制复用思考语言） | 小批 |
| 批3 指令文件 | G/H 发现 + 头部合成消息注入 + 快照锁字节 + 设置开关 + 幻影回归 | 新能力，中批 |
| 批4 尾部碎片 | 包裹/转义工具 + 三碎片（整理完成/Plan/到顶）+ 落点裁决（kernel vs prepare） | 新机制，中批 |

每批独立可交付；批1 不依赖后三批，先治用户痛点。

## §9 已知边界

1. 提示词全部英文（与现行块一致），回复语言靠 E 节「跟随用户」——思考语言中间件管思考侧，不冲突。
2. 静态段跨会话同前缀 = 供应商前缀缓存跨会话命中成为可能，但 Locowrk 不做跨会话缓存观测承诺（供应商侧行为）。
3. 指令文件写坏（超 100KB/二进制）= 截断 + 标记如实，不静默丢弃。
4. 尾部碎片若与用户消息同轮密集触发（整理完成 + 切 Plan），合并为一条多段 reminder，不拆多条。
5. 批3/批4 落点（kernel messages 可变性）待实现时查证，§6 已列两条路径。
6. 「背景不是议程」的措辞强度需实机观察：过弱则开场仪式复发，过强则该查记忆的场景也不查——批1 走查以「闲聊零仪式 + 让它查历史偏好时一步到位」双场景验收。
