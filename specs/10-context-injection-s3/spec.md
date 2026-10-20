# 10-上下文注入面 S3（索引常驻、正文按需 + 占用读数一嘴到底）

- 状态：**进行中 · 刀①②③a③b④a ✅**（开工三问由决策人 **2026-10-20「三问照推荐案，开跑」** 一并
  采纳，裁定结果记在下面「开工三问与裁定」那节；本文件是
  [`specs/01`](../01-memory-context-redesign/spec.md) 阶段表里
  **S3 注入面** 那一行的施工 spec；方案稿对应 = **§C**（+ §A④ 索引制、§F 交接层），
  ⊘ 不入库、只在作者本机，关键条款抄在下面「取证」与「目标」里；决策依据 =
  [`ADR-0007`](../../docs/adr/0007-memory-flat-files-context-llm-compaction-primary.md)）
- 负责人：Qoder（取证 + 实现） / 青鸟（下面「要你定的」三处点头）
- 日期：2026-10-20（现状取证同日）
- 出处 / 上游：方案稿 §C「注入面：索引常驻，正文按需」+ [`specs/01`](../01-memory-context-redesign/spec.md)
  的 AC-4 / AC-5 / AC-7（本线吃这三条）+ S1 留给本线的两格（
  [`specs/05`](../05-memory-data-plane-s1/spec.md) 非目标第 1 条「注入面形状 = S3」与风险③
  「mtime 进预取缓存键 = S3 的接线」）+ S2 长跑落下的账 ⓟ（
  [`specs/09`](../09-context-compaction-s2/spec.md) 挂账区 ⓟ 那一条；**不钉行号**——
  那份文件每补一段记录行号就漂，2026-10-20 本线就在它里面写了四条关闭说明），
  **ⓟ 按决策人 2026-10-20 指令
  归进本线刀④**，不再单立一刀。

## 目标

两件事，都是"把形状扳到设计稿那一格"：

1. **注入面**：现在注入的是「全局事实 top-K **正文**（还有一条按 sqlite 加权的通路）+
   项目索引指针行」两段分开；目标形状 = **两段索引指针行**（全局 `MEMORY.md` + 项目
   `MEMORY.md`，钳 200 行）常驻，**正文一个字都不注入**、模型按需读文件。连带三件：
   人改 `MEMORY.md` 下一轮就生效（mtime 进预取缓存键）、`<memory_guidance>` 按新工具面
   与排除清单改写（它现在还在承诺树工具）、树目录段随本线移除。
2. **占用读数一嘴到底**（ⓟ）：压缩面判断到线用的是**三档取最大**，界面进度条用的是
   **另两档**——同一个"占用"在同一个进程里有两个算法，失败期间界面停在旧数；而
   「输出预留 ≥ 一轮压缩增量」这条关系界面上没有任何一格说破。这一格要收成**一个出处**
   并把扣减链**显形**。

## 取证（2026-10-20 读实，逐条带出处；这一节决定下面每一刀的形状）

1. **注入面今天是五个段**，组装全在 `crates/kernel-agent/src/middleware/memory_inject/mod.rs`：
   `<environment>`(:255) → `<repo_context>`(:259) → `<persistent_memory>`(:164/182，
   由 `recall_relevant` 出条目 :276-280) → `<project_memory_index>`(:285) →
   `<memory_guidance>`(:85-99 内核常量)，另有一族**追加**在后的树目录段
   （`crates/kernel-agent/src/middleware/tree_context.rs:24` `TreeDirectoryInject`，
   内文由宿主 `desktop/src-tauri/src/organize/assemble.rs:191/203` 生成）。
2. **数据源是三种混着**（宿主实现 `desktop/src-tauri/src/memory/source.rs`）：
   `<persistent_memory>` **两档**——`memory.learnEnabled` 开 = 走 sqlite 四信号加权
   （`memory_learn::candidates`，`source.rs:97-106`），关 = 走 md 文件
   （`memory/recall.rs:81-99` `memlayout::iter_facts`，**只全局层**）；
   `<project_memory_index>` = md（`source.rs:217-218` → `memory_index.rs:66 from_md`）。
   注释自己说破了处境：`source.rs:11`「本片是宿主读面里**最后一条仍按库寻址的通路**」。
   ⇒ S1 把真源扳到 md 之后，`learnEnabled` 那一档是**残留的第二真源**。
3. **全局 `MEMORY.md` 从没进过注入面**：`grep global_index|global/MEMORY desktop/src-tauri/src`
   零命中。设计稿 §A④ 要的"每会话注入两个索引"现在只有项目那一个。
4. **`<repo_context>` 是纯静态文本**：`desktop/src-tauri/src/repo_context.rs:69`
   `pub fn build(_repo: &Path)` 参数不消费，返回固定串（文案点名 `requirements.md` /
   `handoff.md` / `notes/`，还写着 "These files are NOT auto-injected"）。§F 要的
   "`<repo_context>` 携带 `MEMORY.md` 正文 + requirements 提醒行"**没接线**；
   `handoff.md` 这个旧名字还活在文案里（§F 已把它换成 `MEMORY.md` + `journal/`）。
5. **预取缓存键不含 mtime**：`memory_inject/mod.rs:229-232` 键 =
   `session_id + '\u{1}' + scope_id`，条目形状 `{ block, top_k }`（:101-135），失效通路只有
   换会话 / 换键位 / `top_k` 变三种；`source.rs:147-151` 的注释明写「交接文件 mtime 指纹
   **退出**缓存键」。⇒ 人改 `MEMORY.md` 在同一会话内**不生效**（S1 风险③ 登记的就是这一格）。
6. **`<memory_guidance>` 现文在承诺退役物**：`mod.rs:85-99` 里写着「Empty results …
   do NOT mean there is no history: project history lives in the memory tree — call
   `memory_project_history` first, and `memory_search_titles` for topics」。这两枚工具
   （`organize/tree_tools.rs:44`、`organize/search_titles.rs:43`）是 §D 点名要退役的
   树工具族，归 S4。⇒ 本线要么同刀改掉承诺，要么留下"指引指向活物、S4 一拔就指空"的债。
7. **硬钳有唯一出处但只施在项目段**：`crates/plugin-api/src/memlayout/index.rs:30`
   `MAX_INDEX_LINES = 200` / `:32` `MAX_INDEX_CHARS = 25_000`（写侧钳 :43-49，落盘 :114-118）；
   注入侧 `desktop/src-tauri/src/memory_index.rs:36` 再导出、`:173-191 clamp` 施在
   `<project_memory_index>` 的指针行上。`<persistent_memory>` 走的是**另一把尺**
   = 条数 `top_k` + 单条 500 字符（`memory_inject/mod.rs:67` `VALUE_INJECT_MAX_CHARS`）。
8. **顺序被钉住的部分**：装配点唯一 = `desktop/src-tauri/src/state/waterfall.rs`
   （:5-8 头注「注册序就是执行序，一条不许重排」，:108 `MemoryInject` → :132
   `TreeDirectoryInject`）；`crates/kernel-agent/tests/prompt_layering.rs:74-101` 钉
   「静态前缀 → 记忆居中 → 目录殿后」与「目录增长不破前缀」，五段**内部**序由
   `memory_inject/tests/project_index.rs:89-95` 钉。`scripts/check-middleware-wiring.mjs`
   只查"有定义无构造点"，不查顺序。
9. **ⓟ 那一格的事实（三条，全部读实）**：
   ① 分子两副算法——界面 = `desktop/src-tauri/src/runner/mod.rs:370-403 record_usage`
   （:378 只吃 `reported_input_tokens`，无上报才回落估算），压缩面 =
   `desktop/src-tauri/src/compact/deep.rs:117-131 Compactor::used`（上报 / 压后投影估算 /
   拒收读数**三档取最大**，:130 `max(refusals.of(...))`）。两副算法**没有共同出处**，
   且 `used()` 生产调用点只有一个（`deep.rs:178`）、**对界面零暴露**（`/压缩` 的返回值
   `commands/compact.rs:39-42` 只有 `{ compacted, reason }`）。
   ② 通道只有一条：`SessionEvent::ContextUsage` → `AgentEvent::ContextUsage` →
   `agent://context-usage`（`events.rs:23/387`）→ `stores/chat/projection.ts:211`；
   `ContextCompacted` 带 `from_tokens/to_tokens` 但**不带当前占用**（`events.rs:178-185`）。
   界面三处读同一份 `contextUsage`：`ContextProgressBar.vue:19-34`（`used/threshold` 与
   `used/windowTokens`）、`UsageRing.vue:22-26`、`ContextUsagePopover.vue:47-60`。
   ③ 扣减链在界面上是**暗的**：`context_budget.rs:25/32` 那两颗常量（输出预留封顶
   21 000 / 缺省 32 000）与 `:73-82 effective_threshold` 的式子，界面零呈现；
   输出上限本体住**模型**分区（`settings/model/ModelPanes.vue:505-511 m.maxOutput`），
   和「压缩阈值」不在同一屏 ⇒ 用户看不见"这两枚相减才是要守的那条线"。
   ④ 本仓**没有**"A 必须 ≥ B"这类跨字段校验的先例（`settings/context.rs:265-353
   apply_update` 全是单字段区间判定）。可借的既有形态只有三种：纯文案描述关系
   （`locales/zh-CN/settings.ts:347-349/372-373`）、后端 `Err` → 板面级错误条
   （`settings/context.rs:271-277` + `SettingsSaveRow.vue:5`）、设置页**只读运行时读数块**
   按态换色（`DeveloperSection.vue:246-330`）。

## 验收标准（必须可测：跑什么、看到什么）

- [ ] AC-S3-1 两段索引指针行上界面、旧两块下线：真窗口跑一轮，开发者面板/日志的
      `agent/request` 里 system 带**一个**索引块（含全局段 + 项目段），段里每条指针行
      能在 `~/.Locowrk/memory/` 下指到一个真实文件一行（= specs/01 的 AC-7 可追溯）；
      `<persistent_memory>` 与 `<project_memory_index>` 两个标签在生产面**0 命中**
      （新判据 = 后端标签表 ↔ 扫描面读数，配变异）。
- [ ] AC-S3-2 正文一个字都不注入：夹具里放一条有长正文的事实 ⇒ 注入块只出现它的
      指针行，正文串在 system 里 0 命中；`top_k` 不再管正文条数（其语义要订正或摘除）。
- [ ] AC-S3-3 人改即生效（mtime 接线）：**同一会话内**改 `global/MEMORY.md` 加一行 ⇒
      下一轮 system 带上那一行；同会话**不改** ⇒ 该段逐字节相同（证缓存仍命中、
      前缀不被砸）。两条都要用例，缺一条不算。
- [ ] AC-S3-4 `<repo_context>` 换成读真实交接层：注入内文来自 `<repo>/.Locowrk/MEMORY.md`
      正文（不是静态串），文案不再点名 `handoff.md`；文件不存在时**整段缺席**而不是
      注入一句"还没有"。
- [x] AC-S3-5 指引改写不自相矛盾（✅ 2026-10-20 刀③a）：四要点全在场（排除清单四条是
      旧版完全缺的那一格），§D 那四枚退役工具名与"memory tree"承诺在**交给模型读的文案面**
      零命中；判据 = `SS28` 两段读数实测 `0|2`（① 退役名 0 / ② 新要点恰 2，防"删光文案
      也算通过"），`make selftest` 给 `✓ SS28 变异可见`。
- [x] AC-S3-6 移除不留死码（**拆两半都完成**）：① 树目录那半 ✅ 刀③a——`TreeDirectoryInject`
      连定义带注册一并摘除；`prompt_layering.rs` 三条用例不是"删两条留一条"就完事：留下的
      那条改证"静态身份段恒为 system 字节前缀"，被删那条的**纪律继承者**写进文件头注与
      提交正文，`kernel/tests/tree.rs` 的断言反转成"plan() 交了目录也不许进上下文"。
      ② 学习档那半 ✅ 刀③b——`memory_learn/` 整族、契约侧 `recall_relevant` /
      `LearnWeights` / `RecallQuery` / `LEARN_WEIGHT_DEFAULT`、`MemoryInjectConfig.learn`、
      `VALUE_INJECT_MAX_CHARS`、设置面 `learnEnabled` + 四枚权重（后端字段 / 校验 / 投影 /
      前端那一格 / 中英两条文案）、`breakdown` 观测通路（写侧参数 → DDL 列 → 迁移件 →
      解释面板 → 前端 `explain` 区块）**一条不留**；**影子库 `hit_count` / `last_hit_at`
      两列按清点不动**（S4 的 sqlite 退役面），连同 `memory_inject_log.breakdown` 那列一起
      登记为「在场、无人读写」。审计报告那条「存在但从未接线」现在由判据 `SS29`/`SS29b`/
      `SS29c` 三档机械挡住（含 ③ 那档正对照，防"删光也算通过"）。
- [x] AC-S3-7 占用只有一个出处（ⓟ 本体）**✅ 刀④a 2026-10-20**：界面分子与 `Compactor::used`
      **同一个函数** = `context_budget::occupancy(session, refused)`（三档取最大：新鲜上报 /
      上报过期时的全量估算 / 被超窗拒收那颗数）。判据两档 = `SS30`（① 定义只住一个文件
      ② 调用恰 2 处 ③ 装配口 `assemble` 只住两个组装点）+ `SS30b`（`UsagePolicy {` 字面量与
      `.context_tokens(` 在生产面各恒 `none`）；**"任一侧单独改尺即红"落在 ② 这一档的读数上**：
      它数的是两处的总调用数，进度条或压缩面任何一侧换成别的尺 ⇒ 从 2 掉到 1，多开第三个
      消费点 ⇒ 从 2 涨到 3，两个方向共用同一格读数（变异自证 `make selftest` 两条都报红）。
      构造"上一轮被超窗拒收"的会话态 ⇒ 事件里的 `used` ≥ 拒收那颗数：这条由
      `context_usage_takes_the_refusal_reading` 落地（上报 95 483 + 拒收 240 000 ⇒ 每条
      `ContextUsage.used` = 240 000 而 `threshold` 照旧 8 000——分子顶过线不改分母）。
      ⚠ 本条改的是**读数口径**不是修 bug：无上报的树会话进度条从此按全量历史估算（只会变高），
      方向与代价写在 `context_budget.rs` 头注与下面刀④a 施工记录第 3 条。
- [ ] AC-S3-8 扣减链显形（ⓟ 的另一半）：式子的真源 = `context_budget::effective_threshold`
      （`= min(滑杆值, 窗口 − output_reserve)`，`output_reserve` = `min(输出上限 ?? 32K, 21K)`，
      **只扣一次**——AC-S2-2 的靶就是扣两遍；
      **引用符号名而不是行号**——刀④a 在本文件里加了 `occupancy`，行号当场漂了，而这一条
      是刀④b 要照着动码的，钉行号等于给下一刀留假证据。取证 9③ 那处行号**保留**，
      它是 2026-10-20 当时的读实证据，改了就失真）。
      界面要能一眼读出这条链：`窗口 → −输出侧预留 → 与滑杆取小 = 有效阈值 → 已用 / 有效阈值`，
      外加一格「窗口 − 有效阈值 = 留给输出与下一轮净增的空位」——**这颗空位就是 ⓟ 说的那条
      关系**（净增超过它 ⇒ 撞窗）；输出预留那颗数必须是**带出处的读数**（写出"21K 封顶"或
      "按你钉的输出上限 8K"）而不是隐藏的常量。逐态写清各看到什么：窗口取不到（`None` 那一支，
      阈值原样吃滑杆）/ 窗口取到且滑杆更小 / 窗口取到且被预留封顶（滑杆说了不算）。
      用户把输出上限钉大钉小 ⇒ 这一格当场跟着变（`min` 的两侧都要有用例反证，
      沿 `reserve_follows_a_small_ceiling` 那条既有纪律）。
- [ ] AC-S3-9 `make check` 全绿；涉界面两刀（刀①④）各自在真实窗口自截图核对后再给你看。

## 非目标

- 不做回合收束的增量抽取（S1 之后单立一批，要 LLM 通道）；不做 embedding 检索
- 不动 organize 管线、不退役 memory-sqlite 插件与树工具族本体、不动 `MemoryPage`
  两个 tab、不建 §E 那三枚行为开关（全归 S4）
- 不动 ⓖ/ⓗ 两枚控件形态（仍等你批形态，本线不借道塞进去）
- 不合并"生效窗口"两条链（specs/09 取证 6 登记的既成事实）
- ⚠ 这一条原来还写着"**不动账 ⓛ（定时那条链）**"，**刀④a 之后订正**：ⓛ 被顺带关掉了，而
  这不是主动扩大——`UsagePolicy` 多了第四枚原料（`refused_tokens`）之后，两个组装点各手写一遍
  字面量就从"分母可能漂"变成"分子也可能漂"（定时链的会话同样撞超窗）。收成
  `UsagePolicy::assemble` 的净代价是 −12 行，比"两处各补一枚字段"更少。写在刀④a 施工记录第 5 条。
- 不做跨设备同步、不做应用内记忆列表视图
- ⓝ（某类会话里深度压缩长期零起步）与 ⓞ（判据口径）**不归本线**：一条是走查读数说明、
  一条已写进 spec

## 计划（四刀，每刀一提交、跑绿再走下一刀）

1. **刀① 索引段替换** ✅ 2026-10-20：新块（全局段 + 项目段，钳 200 行 / 25 000 字符，复用
   `memlayout` 那颗常量与 `memory_index.rs` 的钳）；`<persistent_memory>` 与
   `<project_memory_index>` 两块下线；标签判据 + AC-S3-1/2。**施工记录见下面那节**。
2. **刀② 缓存键与注入源** ✅ 2026-10-20：mtime 进预取缓存键（全局 + 项目 + 交接层三处指纹）；
   `<repo_context>` 从静态串改成读 `.Locowrk/MEMORY.md`；AC-S3-3/4。**施工记录见下面那节**。
3. **刀③ 指引与退役段**（按上面那节清点拆成两步）：
   **3a ✅ 2026-10-20** = `<memory_guidance>` 与两枚导语改写（补排除清单、空结果换指向、
   去掉对退役树工具的承诺）+ `TreeDirectoryInject` 摘除 + 判据 `SS28` 新增 / `SS8c` 名单
   显式改；**3b ✅ 2026-10-20** = 学习档族删除（`memory_learn/` 933 行 + 20 处触点 + 设置面
   与前端那一格 + `breakdown` 观测通路；影子库 `hit_count` / `last_hit_at` 两列留 S4）
   + 墓碑判据三档 `SS29`/`SS29b`/`SS29c`；AC-S3-5 ✅ / AC-S3-6 ✅。
   **刀① 留下的一格连带后果在本刀登记为新欠账 ⓠ**（注入侧埋点失去生产调用点），见下面
   刀③b 施工记录第 5 条。
4. **刀④ 占用一嘴到底（ⓟ 归这一刀）**：按"后端一个出处"与"界面上看得见"拆两步——
   **4a ✅ 2026-10-20** = `record_usage` 与 `Compactor::used` 同吃下沉出来的
   `context_budget::occupancy`（不是"进度条改吃 `Compactor::used`"：那样压缩层就拥有了一把
   尺，两个消费点还得互相引用）+ 拒收读数经 `UsagePolicy.refused_tokens` 进进度条 +
   `tree` 参数整链摘除 + 装配口 `assemble`（顺带关闭 specs/09 挂账 ⓛ）+ 判据 `SS30`/`SS30b`；
   AC-S3-7 ✅。
   **4b（待做）** = 扣减链上界面（`窗口 → −输出侧预留 → 与滑杆取小 = 有效阈值 → 已用/阈值`）
   + 输出预留那颗数做成可调项（三问裁定 ③）；AC-S3-8。
   ⚠ 原计划里"拒收读数进 `ContextUsage` 载荷"这一格 **4a 没做，且决定不做**：分子已经是
   `occupancy` 的结果，事件里那个 `used` 就含拒收那一档；再单独传一颗数是"把同一个数的两个
   来源都摆上界面"，而 4b 要显示的扣减链是**分母**那条式子，不是分子的第 3 档。

## 刀① 施工形状（本线第一刀；与三问无关，点头即照此动码）

- **契约**：`MemorySource` 新增 `global_index(session_id) -> Option<String>`，**缺省 `None`**
  ⇒ 五个测试 fake 与 headless 装配零改动（与 `repo_context` / `project_index` 同一惯例）；
  `project_index` 语义一字不动。内核把两段包成**一个** `<memory_index>` 块，段头
  `[global]` / `[project]`，两段各自可缺席（由 `append_block` 的"add=None 不动"承担，
  不写第二种早退分支）。块名与段头是我定的，可翻案，不为它单开一问。
- **同刀删**：`build_block` / `clip` / `tag_suffix` 三个私有件——摘掉 `recall_relevant`
  那唯一一次调用（`mod.rs:276-280`）之后它们必成 dead code，`clippy -D warnings` 当场红，
  所以删它们是这一刀的必经之路，不是"顺手清"。
- **同刀不动**：`recall` / `recall_relevant` / `MemoryEntry` / `VALUE_INJECT_MAX_CHARS`
  全部原样留着——宿主 `relevance` 信号还在复用那颗常量（`mod.rs:62-67` 写明了"匹配面必须
  等于注入面"这条纪律与它的判据），而 `learnEnabled` 一族的去留是**三问②**，不在本刀偷做。
- **`top_k` 只改一处**：早退条件从 `!cfg.enabled || cfg.top_k == 0`（`mod.rs:215`）改成只看
  `enabled`——索引常驻之后"条数设 0 连索引一起关掉"是说不清的隐藏耦合。但
  `MemoryInjectConfig.top_k` 字段、`settings/memory` 那三处、`PrefetchEntry.top_k`
  **一律不动**：`settings.rs:279-287` 那段注释把"字段集只有 `block`/`top_k`"写成**有意承重**
  （破它 = 有人能把权重塞进失效判据），要改得单独带理由。登记：`memory.topK` 自本刀起
  在注入面**无消费者**，去留随 §E 那一批（S4）。
- **宿主侧**：`global_index()` 读 `~/.Locowrk/memory/global/MEMORY.md` 的**落盘正文**
  （真源 = 文件、不现算排序 = specs/01 的 AC-7）；钳制复用 `memory_index.rs:173 clamp` 与
  `memlayout/index.rs:30` 那颗常量——**同一把钳不许出现第二个出处**，全局段与项目段
  走同一个 `clamp`，只是喂进去的 body 不同。
- **判据**：新 SS 项钉「`<persistent_memory>` 与 `<project_memory_index>` 两标签在生产扫描面
  0 命中」+「`<memory_index>` 恰好 1 处」；变异必须是**在生产文件里**复活旧标签
  （测试夹具里出现旧标签不算红，否则这条判据挡不住"生产还留着、只是没人调"）。
- **用例**：`memory_inject/tests/project_index.rs:89-95` 那条「facts < index < guide」的内部序
  按新形状改写（旧的两块没了）；`crates/kernel-agent/tests/prompt_layering.rs` 两条**不动**
  （环序没变）；`tests/prefetch.rs:80-101` 的「top_k 变 = 重算」仍绿（字段没动）。

## 刀① 施工记录（✅ 2026-10-20 · 22 文件 +622 / −862）

按上面那版形状落地，**四处偏差与连带**都要记（都是"不这么做就当场红"的那种）：

1. `terms.rs`（107 行）与 `tests/learn.rs`（7 条用例）**随唯一消费者同刀离场**：原计划
   只写"摘调用点"，实际不摘这两件，`clippy -D warnings` 的 `dead_code` 当场红。
   learn.rs 里那条 M1 承重（"同会话只取数一次、不许把权重塞进失效判据"）**迁到
   `prefetch.rs`**，道具从"正文条目漂移"换成"索引内文漂移"，断言语义一个字没改；
   `settings.rs::memory_inject_config` 头注点名的是 learn.rs 那条 ⇒ 注释同步订正并留
   "原先住哪"的记号（不改它 = 留下一条指向已删用例的假指针）。
2. `top_k` 早退条件按计划改掉，并**新增一条用例** `zero_top_k_no_longer_switches_the_block_off`
   钉这次语义变更本身（旧形状下 `top_k = 0` 关整块，现在不关）。
3. **观测通道同步**：`llm/diagnose.rs` 的 `SYSTEM_MARKERS` 从 6 枚改 5 枚（旧两块合并），
   顺序跟着新组装序走；它的夹具与断言（`diagnose/tests.rs`）同步换成 `<memory_index>`。
   漏改这一处的症状不是编译错，而是"索引段中途变化被报成 system 变了"——缓存诊断
   从此看不见这一格，而刀② 要接 mtime 时最需要盯的就是它。
4. 内核侧三处假件换通道：`memory_inject/tests/mod.rs`（`Fixed` → `Empty` + 新增 `Sourced`）、
   `kernel/tests/mod.rs` 的 `DriftingMemory`（`recall` 漂移 → `global_index` 漂移）、
   `kernel/tests/prefix.rs` 与 `tree.rs` 的定位标签。
5. **判据 `SS27`**（三段读数：旧标签 0 命中 / `format!("<memory_index>` 恒 1 / 字面量所在
   文件集合 = 内核 + `llm/diagnose.rs` 两处）实测读数 = `0|1|...mod.rs|...diagnose.rs`；
   窄变异 = 在生产面复活 `<persistent_memory>`，`make selftest` 读数 `✓ SS27 变异可见`。
6. **真机数据根核对**（不是假件）：`~/.Locowrk/memory/global/MEMORY.md` 在场，形状
   `# 记忆索引` + 一条 `- [user.pref.techstack-agnostic](user.pref.techstack-agnostic.md) — 技术栈无关 规范设计`
   ⇒ 与 `pointer_lines_from_md` 的解析形状匹配；全局层目录经 `memlayout::layers()` 枚举，
   宿主没有第二处 `global/` 字面量。
7. **门禁**：`make check` 末行 `▶ check 全绿`（宿主 709 条 / 内核 98 条 + 全部集成套件 /
   vitest 193 条 / 两处 clippy 干净）；`make selftest` = `SELFTEST_ALL_CAUGHT`。

**这一刀欠的一发读数**：AC-S3-1 里"真窗口跑一轮看 system"未取——刀① 没动任何界面组件，
而真窗口那趟要看的 `repo_context` 还在刀② 改，所以这一发**随刀② 一起补**（届时一次
起窗口取两刀的证）。登记在册，不算已验。

**新增的挂账**：`MemoryInjectConfig.learn` 那枚字段现在**无读者**（内核不再读它，
宿主 `memory_inject_config` 仍在填）——它随刀③ 的 `learnEnabled` 族一起清，本刀不偷删
（删它要动 settings 面，属刀③ 的改动面）。

## 刀② 施工记录（✅ 2026-10-20）

**键**：预取缓存键从 `(session_id, scope_id)` 变三段 `(…, index_stamp)`。契约新增
`MemorySource::index_stamp()`（缺省空串 = 不参与键），宿主实现交 `g{ms}p{ms}r{ms}`
三档（全局索引 / 项目索引 / 交接正文），**只 stat 不读内容**（它每 step 都跑，排在缓存
查询之前）。三处路径全部复用既有单点算式（`memlayout::layers` / `project_dir` /
`MemoryStore::repo_root` + `repo_context::{DIR, HANDOFF}`），宿主没有再拼一遍 `global/`
或 `MEMORY.md` 字面量；`HANDOFF` 抬成 `pub(crate)` 的理由写在它自己的注释里。
`scope_id` 与 `index_stamp` **分成两枚方法**而不是合成一个串：键位身份与新鲜度是两件
事，混在一个返回值里就没法分别判读（`scope_id` 那条"mtime 退出缓存键"的旧注释同步订正）。

**注入源**：`repo_context::build()` 从静态串改成读 `<repo>/.Locowrk/MEMORY.md` 正文 +
三行维护提醒。三个决定要记：① **读不到 ⇒ 整段 `None`**，不退回旧的静态提醒——留一份
与文件无关的提醒会把"这层没接线"与"接了但暂时没内容"混成同一个形状；② 因此
`ensure_layout` 同刀多写一份 §F 主题分节骨架（否则第一个会话什么都收不到，"注入源切换"
就成了空头话），骨架与 requirements 模板都是"缺则写、有则一个字不碰"；③ 正文按 §F 硬顶
做行 + 字节双钳，**超预算那一行整个吐掉**（半行会被模型读成一条完整事实）。旧静态串里
那三个退役物（`handoff.md` / `notes/` / `memory_project_history` 等三枚树工具）随文本
一起消失。

**一处语义放宽，不许说成"延续 M1 纪律"**：旧形状下模型自己 `memory_write` 之后本会话
看不见自己写了什么（那是刻意的，防逐轮换字节砸前缀）；mtime 进键之后**分不清改的是人
还是模型** ⇒ 模型每写一条事实，下一轮付一次前缀缓存断裂。买到的是"人编辑 `MEMORY.md`
当场生效"（§C/§F 硬要求）+ 新记的那条本会话内可见；付掉的是每次写入后一轮全价 input
（与 S2 刀④ 主层压缩那种一次性前缀重置同量级，不是每轮都付）。这段记在
`memory_inject/mod.rs` 头注里，不在文档里粉饰。

**删掉一条用例**：`repo_context/tests.rs::build_is_static_reminder_independent_of_files`
断言的正是本刀要**反转**的形状（"与文件状态无关"）。删它不是删测试凑绿，替代它的断言
写在同一簇里（"改了正文输出必须跟着变"是同一格的反面证据），删除理由记在该文件头注。

**用例**：内核 1 条正反同体（`a_fresh_stamp_recomputes_while_a_stale_one_freezes`：
不变 ⇒ 逐字节复用且取数次数不涨；变 ⇒ 重算、带新内文、旧份消失、新键下重新钉死）；
宿主 2 条（`the_index_stamp_follows_each_injected_input_and_only_that_one` 钉"只有该动的
那一档动"——防止过度失效砸前缀；`an_unattributed_session_still_carries_the_global_stamp`
挡住"在开头顺手整体早退"）；`repo_context` 3 条（骨架幂等且不覆盖人写的 / 正文进段 +
缺席 + journal 不注入 + 退役名不在场 / 超预算钳 + "没超就不许说超了"的负对照）。

**门禁**：`make check` 末行 `▶ check 全绿`（宿主 711 条 / 内核注入簇 21 条 /
vitest 193 条 / `SELFTEST_ALL_CAUGHT`）。首跑在 clippy 红过两条——
`explicit_counter_loop`（钳制函数拿 `lines` 当计数器）与 `&PathBuf` 该写 `&Path`，
两条都是真信号，改的是写法而不是加 `#[allow]`。

**这一刀新增的挂账**：
- ⓠ 交接层 `MEMORY.md` 的**写侧钳不存在**（S1 只给记忆库两层做了写侧钳）⇒ 今天只有
  读侧钳在挡，§F 那句"超顶 = 逼蒸馏"没有生产者；补它要动收束更新那一侧，不属注入面。
- ⓡ §F 的另两件没跟着落地：`journal/<YYYY-MM-DD>.md` 的**按天封卷**与"轮次收束后模型
  主动更新"的提示词改写。刀② 只切了注入源（本刀的承诺面）。存量 `handoff.md` 归并
  **不需要做**——本机实测本仓 `.Locowrk/` 从未存在过，没有存量可归并（这条不是欠账）。
- 真窗口那一发（AC-S3-1/3/4 的"跑一轮看 system"）仍欠，见下面风险③ 的登记。

## 刀③ 拆成两刀的清点（2026-10-20 读实，这一节决定下一轮从哪儿动码）

清点结果推翻了"刀③ 是一刀"的写法，两处非做不可的拆分理由都记在这里：

1. **`tree_directory` 不只是目录文本**。`TreePlan`（`kernel/services.rs:145`
   `TreeContextSource::plan`）一次交出三样东西：目录文本、**树会话标记**、
   `tail_user_start`（消息切到"未整理尾巴"的起点）。内核组装侧
   `kernel.rs:215-227` 拿它做的是三件事，而生产者在
   `desktop/src-tauri/src/organize/assemble.rs` —— **那是 S4 要整片冻结退役的地方**。
   ⇒ 本线只能摘**监听器**（`middleware/tree_context.rs` + `waterfall.rs:132-135` 那一环
   注册），`RequestPayload.tree_directory` 字段与 `plan()` 通路**一律留着**（S4 随
   organize 一起拔）。刀① 我对 `memory_learn` 的裁定是"不留存在但从未接线"，这一处
   为什么可以留：它的字段仍被消费（marker 与切片），消失的只是"文本进 system"这一档，
   而那一档的**生产者**住在别的阶段。这条边界不写下来，下一刀就会顺手去动 organize。
2. **摘掉监听器会撤掉一条纪律的证明**。`crates/kernel-agent/tests/prompt_layering.rs`
   三条用例（`static_prefix_memory_middle_directory_last` /
   `directory_growth_leaves_prefix_intact` / 空串那条）的对象**就是** `TreeDirectoryInject`：
   它们证的是"append-only 段只动尾部 ⇒ 目录增长不砸前缀"。摘掉注入器之后 system 尾部
   不再有目录，这三条失去对象 ⇒ 同刀删除，但**纪律本身要交代继承者**：静态前缀不被动态
   段改写这件事，此后由 `memory_inject/tests/{prefetch,project_index}.rs` 那几条
   （逐字节复用 + 取数次数）守。这一句必须写进提交正文，否则"删了三条测试"看起来就像
   为了过闸删测试。
3. **学习档删除面实测比裁定时那一行大**：`memory_learn/` 本体 **933 行**，
   `learn_enabled` / `LearnWeights` / `memory_learn` 的触点铺开 **20 个文件**，除后端字段
   与前端那一格外还牵到 `inject_view.rs`（管理页注入视图）、`recall_log` 的 `breakdown`
   埋点面、`memory/schema.rs` 那两列 `hit_count` / `last_hit_at`（影子库的列）。
   ⇒ 拆成 **③a**（指引改写 + 树目录段摘除，约 8 文件）与 **③b**（学习档族删除），
   各自跑绿各自提交；③b 里**影子库那两列不动**（属 S4 的 sqlite 退役面），本刀只摘
   生产引用与设置面，并把"列还在、没人读写"这一格如实登记。

## 刀③a 施工记录（✅ 2026-10-20）

**文案面**：`<memory_guidance>` 按 §A③ 重写，四件事各挡一处实测漂移——索引是指针、
读正文有两条通路（`memory_read` 或按指针打开那篇文件）、写入时机与"优先更新既有 key"、
**排除清单四条**（代码可推导的 / git 可查的 / `AGENTS.md` 与 `.Locowrk/` 已写的 /
手头上这一步的临时状态；旧版一个字没提，于是"刚跑完哪个测试"也会被记成事实）、
空结果的新指向（旧版写"项目历史在记忆树里，先调 `memory_project_history`"）。
两枚导语末尾各写一遍的"空结果 ≠ 没历史"**收进指引说一次**——同一句话在两档导语里各说
一遍就是双出处，而它跟"这一层是什么"是两件事。

**摘除**：`middleware/tree_context.rs` 删除 + `middleware/mod.rs` 的 `mod`/`pub use` +
`lib.rs` 导出 + `state/waterfall.rs` 那一环注册。`RequestPayload.tree_directory` 字段与
`TreeContextSource::plan()` **一律留着**（理由见上面清点节第 1 条：它还管消息切片与
树会话标记，生产者在 organize/ = S4 地盘）。

**测试**：`prompt_layering.rs` 三条用例的对象都是那个监听器 ⇒ 留一条改写成
`static_identity_stays_the_byte_prefix`（只证"静态身份段恒为 system 字节前缀"这半），
另两条随道具删除，**纪律的继承者**（`memory_inject/tests/{prefetch,project_index}.rs`）
写在文件头注里而不是悄悄消失。`kernel/tests/tree.rs` 那条改名成
`tree_session_slices_to_tail_but_no_longer_injects_the_directory` 并把断言**反转**：
FakeTrees 照旧交目录文本，用例现在证"有目录也不许进上下文"，同时保留
`payload.tree_directory.is_some()` 那一格（marker 仍随 payload 走）；
`directory_growth_keeps_system_prefix` 删除，原地留一段指路注释。

**判据两处**：① `SS8c` 的链位名单从 7 条显式改成 6 条——这条判据本来就是为
"改链位必须换人来改判据"而写死名单的，所以改动记在名单自己的注释里；顺手把描述里
"链上七个注入环"那句数字删了（**描述与名单漂开过一回**，改成"条数由名单自己给"）。
② 新增 `SS28`（扫描面 = 交给模型读的三份文案本体、剥行注释；① 四枚退役工具名与
"memory tree" 零命中，② 排除清单句 + `.Locowrk/journal/` 新指向恰为 2 处——只挡退役名
的判据会为"把整段文案删光"而绿，所以必须有正向在场那一半）。
**首跑红的是判据自己**：第一版拿整句 `Not everything belongs in memory` 去比，实测恒 1，
成因是源码里那句被 Rust 续行符 ` \` + 换行折断了 ⇒ 换成不被拆的片段，实测 `0|2`。
两条变异在 `make selftest` 里都 `✓ 变异可见`。

**门禁**：`make check` 末行 `▶ check 全绿`（内核 98 条 + 全部集成套件 / 宿主 712 条 /
vitest 193 条 / `SELFTEST_ALL_CAUGHT`）。

## 刀③b 施工记录（✅ 2026-10-20 · 学习档族同刀退役）

**删的清单**（按裁定② 执行；**代码面** `git diff HEAD --stat` 实测 **59 文件 / +517 / −2660**，
净减 2143 行——这个数不含本 spec 与 roadmap 的文档改动，落地那一发提交 `cbced26`
含文档共 **62 文件 / +710 / −2666**，两个数各说各的事，别当成一个）：

1. **`desktop/src-tauri/src/memory_learn/` 整目录**（8 个文件 · 933 行：`signals.rs` 四信号
   算式、`order.rs` 排序与 E5 保留槽、`explain.rs` 回执 JSON、`mod.rs` 候选 SQL 与
   `reinforce` 写回、四簇用例）。
2. **契约面**（`memory_inject/contract.rs`）：`recall_relevant`（trait 方法 + 那条"缺省实现
   = 等价性装置"）、`LearnWeights`、`LEARN_WEIGHT_DEFAULT`、`RecallQuery`、
   `MemoryInjectConfig.learn` ⇒ 生效配置只剩 `enabled` + `top_k`；`VALUE_INJECT_MAX_CHARS`
   同批摘除（刀① 就说过"唯一使用者是 relevance 匹配面"，匹配面没了它就是纯死 API），
   `middleware/mod.rs` 与 `lib.rs` 的再导出同步瘦身。
3. **宿主实现**：`memory/source.rs` 的 `recall_relevant` 覆写（85 行）摘掉；
   `memory/recall.rs` 的**两层埋点合回一层**（`log_inject_keys` 与它的
   `breakdown: Option<&str>` 参数收回——没有第二个 caller 的恒 `None` 参数就是死 API 形状）。
4. **观测通路**：`recall_log` 的 `Row.breakdown` / `note_inject` 第七参数 /
   `insert_row` 两条 SQL 的第九列 / `RecallRow.breakdown` / `#[allow(clippy::too_many_arguments)]`
   （**七个参数变六个 ⇒ allow 连它那段理由一起摘**，留着就是把"这里曾经是七个"当成当前事实）/
   `schema.rs` 的 `ensure_breakdown_column` 迁移件与 DDL 里那一列。
5. **面板**：`inject_view.rs` 从 167 行缩到 60 行——`InjectExplain` / `ExplainRow` /
   `arr4` / `explain_of` / "往回扫 50 行找有回执的那一条"整块离场，`recent()` 与 `RecallRow`
   因此**没有生产读者** ⇒ 降进 `#[cfg(test)] impl`（而不是戴一句 `#[allow(dead_code)]`
   假装活着）；`InjectView` 载荷只剩 `overview`；`commands/memory.rs::memory_inject_view`
   名字保留（改它要动 IPC 契约 + 前端 api + `check-commands` 三向闭合，收益只有一句更准的名字）。
6. **设置面**：`MemorySettings` / `MemorySettingsView` / `MemoryUpdate` 三张形状各删五项，
   `LEARN_WEIGHT_MAX` / `check_learn_weight` / `learn_default()` 三个件删除，`apply_update`
   里那 24 行校验应用段摘干净，`settings.rs::memory_inject_config` 的 `learn` 换算删除。
7. **前端**：`MemorySection.vue` 那一格开关 + 四枚权重输入（模板 68 行 + form/载入/保存
   各 5 行）、`types/memory.ts` 的 `learnEnabled` / 四枚 weight / `InjectExplain` / `InjectRow`、
   `InjectSection.vue` 的占比与权重读数、中英两条 locale 各删 11 键（`check-i18n` 全程绿 = 成对删净）。

**两件有意留下的东西**（不是漏删）：① `MemorySource::recall` 与它身后的 `recall_quiet`
注入埋点；② 三列遗物 = `memories.hit_count` / `memories.last_hit_at` /
`memory_inject_log.breakdown`。**在场、无人读写**，`check-schema-parity.mjs` 的 H1–H7 与
`ensure_learning_schema` 迁移件照旧承重（存量库形状是既成事实，删迁移 = 让旧库停在缺列状态）。
新库不再建 `breakdown` 那一列，老库留着——这条**不是口头结论**：新用例
`recall_log/tests/legacy_table.rs::a_legacy_table_with_the_retired_column_still_opens_reads_and_writes`
拿"退役前的建表串 + 一条旧行"造现场，断言开库、读旧行、按八列写新行三件事都不撞那一列。

**5 ⚠ 本刀查实的一个洞（刀① 留下的，登记为新欠账 ⓠ）**：`recall_relevant` 摘掉之后，
`MemorySource::recall` 在生产面上**一个调用点都不剩**——而它是注入埋点
`recall_quiet → log_inject → memory_inject_log` 唯一的挂点。后果实测得出：L0 那三个指标
（注入次数 / 空块率 / 注入产出率）自刀① 起**不再增长**，管理页那块读数停在刀① 之前的历史值，
`injection_yield` 的注入侧分母从此只由检索回执贡献。这不是"顺手能修"的东西：S3 之后进上下文的
是索引指针行，"这一轮注入了什么"的口径要重定（每条指针算一条？每个会话算一次？空段算不算空块？）。
三条出路留给决策人：**ⓠa** 给索引段埋点（`global_index` / `project_index` 交出指针行时记一行，
口径重写成"注入过几个键位 / 有几段是空的"）；**ⓠb** 观测面板整块退役（连 `recall_log` 注入档
一起摘，检索档留给 ⓜ/ⓝ 那批用）；**ⓠc** 保持现状并在面板文案上如实写"该读数自本版停止累计"。
本刀按"发现问题就登记，不在同一刀顺手改无关面"处理：**`recall` 形状一字未动**，
判据 `SS29c` 钉住它还在（前两档的 0 因此不会与"整条通路被删空"混淆）。

**测试面净变化**：删 4 簇（`memory_learn/tests/` 16 条、`memory/tests/recall_learn.rs` 5 条、
`settings/tests/learn.rs` 4 条、`recall_log/tests/breakdown.rs` 5 条）+ 新增 4 条
（`legacy_table` 1 + `inject_view/tests` 重写后 3）+ 改写 3 簇（`inject_view/tests` 原 6→3、
`inject-section.spec.ts` 三态→两态并**新增逐数断言**、`inject-fixture.spec.ts` 占比算式→
比率自洽）。**`layer_switches.rs` 那条"两档共用一枚门要两条独立红证"没有作废**：第三档
`recall_relevant` 随实现离场，红证改由 `recall` 与 `global_index` 各断一次担（同一条用例内，
所以是"同一个 store 实例上两档同时早退"），原用例位置留了指路注释点名继承者。
宿主 712 → **676** 条、内核 98 条不变、vitest 193 条不变。

**判据三档** `SS29` / `SS29b` / `SS29c`（墓碑）：① 七个名字在全生产 .rs（剥行注释）恒 0；
② 三个形状在 `desktop/src` 的 .ts/.vue（剥 `//` + JSDoc + `<!-- -->`）恒 0；③ 正对照
`fn recall(` 的定义恰在 contract.rs + source.rs 两个文件。三条各配窄变异，`make selftest`
给 `✓ SS29 / SS29b / SS29c 变异可见` + `SELFTEST_ALL_CAUGHT`。
**首跑没有红在实现上，红在判据自己的扫描面**：`types/memory.ts` 的退役注释合法提到
`InjectExplain`，第一版数名字会假红 ⇒ ② 改数**代码形状**并新增 `stripFrontComments`
（前端面此前没有剥块注释的先例——SS6/SS11/SS13 之后这是第四次为同一件事改口径）。
`SS11` / `SS14b` / `check-schema-parity.mjs` 三处描述里那句"`memory_learn` 合法在场"的
**理由已过期**，逐条订正为墓碑口径（形状保留，豁免的理由不再成立）。

**顺带抓出的真实界面缺陷**（本刀重写 `InjectSection.vue` 时由新用例抓出来）：注入产出率那个
读数**在界面上从来没显示过**——组件传的是 `yielded`，两份文案里写的占位符是 `{yield}`，
vue-i18n 对取不到的参数吐空串，于是那句话一直渲染成"注入过的键被用上 （4/5）"。
旧用例只断 title 与占比行有没有上屏，从没断过这个数 ⇒ 缺陷活了至少四个版本。
修法 = 两边统一成 `{rate}`，新用例直接断言 `80%` 上屏。**这不是本刀引入的**，
但它属于 AGENTS §4 那句"截图里一眼能看出的不合理当场修"。

**门禁**：`make check` 末行 `▶ check 全绿`（内核 98 条 + 全部集成套件 / 宿主 676 条 /
vitest 193 条 / `SELFTEST_ALL_CAUGHT` × 三处 / `DOCS_SELFTEST_ALL_CAUGHT` /
`check-middleware-wiring: 监听器实现=9 · 已接线=9 · 未接线=0`）；不可见字符扫描 0 命中。
过程中 clippy 打回三条，都是**删除面的直接后果**而非新写代码的质量问题：两处
`needless_update`（配置只剩两个字段，测试里的 `..MemoryInjectConfig::default()` 失去作用）、
一处 `empty_line_after_doc_comments`（退役记号写成 `///` 会变成下面那个枚举的文档 ⇒ 改 `//`）。

## 刀④a 施工记录（✅ 2026-10-20 · ⓟ 的前半关闭：占用分子一个出处）

**1. 形状：一个函数、两个消费点、一个装配口。**
`context_budget::occupancy(session, refused)` 是占用分子的唯一算式（三档取最大：① 新鲜供应商
上报，经 `anchor`；② 最后一次上报落在最后一次 `ContextCompacted` **之前** ⇒ 改吃全量字符估算；
③ 被超窗拒收那颗数）。两个消费点各剩一行：`runner::record_usage`（进度条 + 落日志）与
`Compactor::used`（压缩判定）。为什么不是原计划那句"`record_usage` 改吃 `Compactor::used`"——
那样这把尺就归压缩层所有，进度条要反向引用压缩面，而 runner 与 compact 的依赖方向在 specs/01
里是平级宿主模块；下沉到中立模块才是搬家而不是换主人（刀① 把 `anchor` 从 organize 搬来是同一条理由）。
`refused` 由调用方喂进来而不是让 `occupancy` 自己去查表：给定（日志, refused）读数唯一，
它才能被纯函数判据钉住；两个调用点若各自再补一次 `max`，SS30 ② 就抓不到了。

**2. 摘掉的整链（`tree` 那把第二尺）。** `TreeAssembler::context_tokens` 是"目录 + 尾巴"
口径的本体，它的**唯一生产调用点**就是 `record_usage` 的树分支，而那条分支的入口是
`run_turn` 的第 11 枚参数 `tree`。所以摘的顺序是反着来的：29 个 `run_turn(` 调用点先各删一枚
实参（用一次性脚本做平衡扫描切参，逐点校验那枚文本形如 `None` / `Some(&…tree_assembler)` 才动，
15 文件），再删签名与 `record_usage` 的参数，再删方法本体，最后删**为它而存在的那串宿主字段**：
`AppState.tree_assembler`、`EngineDeps.tree_assembler`、`spawn_engine` 参数、
`background::Init.tree_assembler`、`memory_stack::Stack.tree_assembler`（内核侧仍经 `Context`
服务表拿 `TreeContextSource`，那是这个类型的正职，不受影响）。判据 `SS30b` ② 从此盯住
`.context_tokens(` 恒 `none`。`organize/assemble/tests.rs` 里那条"组装后 < 全量"的断言同批删除
（它测的就是这个 API），留墓碑注释说明去哪了。

**3. 这是读数口径变更，不是修 bug——方向与代价都要说破。** 无供应商上报的树会话从此按
**全量历史**估算，进度条读数**只会变高不会变低**（"目录 + 尾巴"是它的下界）。变高的后果是
**更早触发**压缩与泄压，而提前压缩 ≠ 超限拒收，方向安全；不变的是有上报那一档（v2.138 那条
"看到的 = 触发用的"从此三档都成立，原先只管"有上报"那一档）。⚠ 待补的读数：真机长会话里
同一轮"进度条百分比"改前改后各是多少——这条要等他开一次真窗口才能取，登记在下面第 7 条与
AC-S3-9 那一格，不用夹具数字冒充。

**4. 新增用例。** `context_budget/tests.rs` 加五档 occupancy 用例：第 1 档**断的是与 `anchor`
等值**而不是手抄 9 500（抄数字的话，把函数换成另一把尺只要给出同一个数就照样绿）；第 2 档
前面还压了一条**夹具自证**（`compaction_really_shrinks_the_projection`——投影没真的变小，
后面那两条"取小者"就是空转）；第 3 档与两条边界（缺席 = 0 不参与、更小的旧拒收不许把读数
压回去）盯的就是"取最大"而不是"有拒收就用拒收"。跨面接线的反证另立一条
`context_usage_takes_the_refusal_reading`：跑真 `run_turn`，断每条落进日志的 `ContextUsage.used`
= 拒收那颗数、`threshold` 不改。为什么纯函数用例不够——改前 `record_usage` 根本没读 `refused`，
算式再对也是零接线（SS26 那条"存在但从未接线"的同款）。

**5. 顺带关闭 specs/09 挂账 ⓛ（原列本线非目标，已订正）。** `UsagePolicy` 多了第四枚原料之后，
两个组装点各手写一遍字面量的性质从"分母可能漂"变成"分子也可能漂"，于是装配口提成
`UsagePolicy::assemble`：算式仍只住 `context_budget`（`effective_threshold` 那一步），装配口
只做"四枚原料 → 一把尺"。两个组装点各剩取数四行；`SS30` ③ 盯住 `assemble` 的调用集合恰在
`commands/chat.rs` 与 `scheduler/engine.rs`。定时链的拒收读数经 `deps.compactor.refusal(..)` 拿
——**没有给 `AppState` / `EngineDeps` 新加一张表的字段**：`Overflow` 表的宿主侧读口从此只有
`Compactor` 一个（新增 `refusal()` + `Overflow::reading()`，把"没有记录当 0"这一裁定收成一处）。

**6. 要说破的遗留：整理那一层仍是两档。** `Organizer::release_pressure` 吃的还是
`context_budget::anchor`（没有 stale 档也没有拒收档），所以"三个消费点一个出处"这一刀只做到
**两个**。不补的的理由写在 specs/09 那条 ⓟ 的关闭说明里：organize 按 specs/01 D 节在 S4 整面
退役，给它加档 = 给要删的模块加深依赖。这一格留着，别把它读成"ⓟ 全关"。

**7. 门禁（本刀实测）**：`make check` 末行 `▶ check 全绿`（宿主 682 条通过 / 0 失败，比改前
多 6 条 = 本刀新增的 5 档 occupancy 用例 + 1 条接线反证；vitest 193 条 / 26 文件全绿；两处
clippy 无告警）。`make selftest` 退出码 0、三处 `SELFTEST_ALL_CAUGHT`，新立两条都在里面
（`SS30 变异可见`、`SS30b 变异可见`）：前者的窄变异 = 把进度条那次调用换成
`anchor(session).0`，只让 ② 从 2 掉到 1，① ③ 不跟着红 ⇒ 证明这一档独有；后者的宽变异 = 一次
push 同时塞回手写字面量与 `.context_tokens(`。`make gates` 退出码 0、12 项全 ✓，
`check-middleware-wiring` 仍是「监听器实现=9 · 已接线=9 · 未接线=0 · 白名单=0」（本刀没动环）。
不可见字符扫描（假名 / 零宽 / NBSP）0 命中。
判据首跑**红在判据自己的扫描面**过一次：`UsagePolicy {` 那一档实测
`commands/chat.rs|runner/mod.rs`，原因是返回类型 `-> UsagePolicy {` 与 `impl UsagePolicy {`
都被数成了"字面量构造" ⇒ 读数前先把三处合法在场剔掉（SS23b"正则要收窄"的同一条教训），
而不是把期望改成那两个文件。

**8. 本刀没碰的**：`events.rs` / `runner/projection.rs` / 前端全部（`ContextUsage` 载荷字段
一字未改，前端读到的 `used` 就是换源后的同一个数）；界面读数等刀④b 上扣减链时一起自截图核对
（AC-S3-9 那一格仍欠，见任务 ⓠ/真窗口那笔）。


## 开工三问与裁定（2026-10-20 决策人「三问照推荐案，开跑」⇒ 三处全按推荐案落地）

① **刀④ 的可见性用哪种既有形态**（本仓没有跨字段校验先例，取证 9④ 已列三种可借）
   —— **裁定 = 案 A**：
   - **✅ 采纳**：进度条浮层加一行只读扣减链（`窗口 → −输出侧预留 → 与滑杆取小 = 有效阈值`，
     式子照 `context_budget::effective_threshold` 不另造），
     设置页「压缩阈值」那行的说明文案跟着引用它——**复用既有的"只读读数 + 文案"形态**，
     不加新控件、不做保存拒绝（拒绝会让用户改不动，且 §4 要求限制可关）。
     代价：关系仍然是"看得见但不拦"，撞窗还得等真撞。
   - **❌ 否掉的 B**：再加一道后端 `Err`（输出上限 × 阈值 跨字段校验，保存即拒）——最硬，
     但它把"预留 ≥ 一轮增量"这条**没有外部真源**的关系钉成禁令，违背 §4「限制可关」。
   - **❌ 否掉的 C**：只做纯文案（现 `thresholdHint` 那种），不加读数——最省，但 ⓟ 要的正是
     "当场看见这一档动了多少"，纯文案给不了读数，等于没销账。

② **`learnEnabled` 那一档与 `memory_learn` 族**在刀③ 摘掉唯一调用点之后
   —— **裁定 = 同刀退役删除**：md 是唯一真源，加权档是残留的第二真源，跨三刀留着
   就是审计报告 H2 判过的那个形状（"存在但从未接线"），且设置页会留一格拨了没反应的开关。
   删除面：`memory_learn/` 整族 + `terms` 面（刀① 已走）+ `learnEnabled` 后端字段与
   设置格 + `MemoryInjectConfig.learn` + `recall_relevant`（trait 方法与宿主实现）。
   **被否的另一案**：登记到 S4 一起拔（代价如上）。

③ **输出预留那颗封顶常量（21 000）**：按 §4「有不确定性的数值一律做成设置项」要不要
   在本线做成可调项（缺省 = 改前那颗常量，越界整笔拒，照刀⑥ 的形状）？还是刀④ 只把它
   作为**只读读数**显形、可调性等 §E/ⓖ/ⓗ 那一批一起做？
   —— **裁定 = 刀④ 做成可调项**（照 §4 那条硬规矩与刀⑥ 先例：后端字段 + 区间常量 +
   越界整笔拒 + 中英两条文案 + **缺省 = 改前那颗 21 000**，等价性用"既有
   `context_budget` 测试一字未改仍然绿"实证，不靠我说）。

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `crates/kernel-agent/src/middleware/memory_inject/{mod,contract}.rs` | 刀①：块形状与标签换成索引段（内核管标签、内容真源在宿主的分工不破）；刀②：缓存键 + mtime；刀③：`GUIDANCE_BLOCK` 改写 |
| `crates/kernel-agent/src/middleware/memory_inject/terms.rs` | 刀① ✅ **已删除**（107 行，随唯一消费者 `recall_relevant` 调用点一起离场；`tests/learn.rs` 那 7 条同刀删，其中的 M1 承重迁进 `prefetch.rs`） |
| `crates/kernel-agent/src/middleware/tree_context.rs` | 刀③a ✅ **已删除**（连同 `middleware/mod.rs` 声明、`lib.rs` 导出、`state/waterfall.rs` 那一环注册）；`RequestPayload.tree_directory` 字段与 `kernel.rs:215-227` 的 `plan()` 带入通路**留着**——它还管消息切片与树会话标记，生产者在 organize/（S4 面） |
| `desktop/src-tauri/src/memory/{source,recall}.rs` `memory.rs` | 刀①：读面新增全局索引段；`recall_relevant` 摘除或保留按裁定 ② |
| `desktop/src-tauri/src/memory_index.rs` | 刀①：全局段复用同一把钳（现在只钳项目段） |
| `desktop/src-tauri/src/repo_context.rs` | 刀② ✅：`build()` 从静态串改成读 `.Locowrk/MEMORY.md` 正文 + 提醒行 + §F 双钳；`ensure_layout` 多写一份骨架；`HANDOFF`/`HANDOFF_MAX_*` 成常量；该簇 3 条用例（含反转掉的那条静态断言） |
| `desktop/src-tauri/src/memory_learn/` | 刀③b ✅ **整目录已删除**（8 文件 · 933 行）；契约侧 `recall_relevant`/`LearnWeights`/`RecallQuery`/`VALUE_INJECT_MAX_CHARS`、设置面 `learnEnabled`+四枚权重、前端那一格与中英文案、`breakdown` 观测通路与解释面板同批离场；`inject_view.rs` 缩到 60 行只管 `overview` |
| `desktop/src-tauri/src/state/waterfall.rs` | 刀③：摘 `TreeDirectoryInject` 一环（头注那"注册序就是执行序"的段要订正） |
| `desktop/src-tauri/src/runner/mod.rs` `compact/deep.rs` `context_budget.rs` `commands/chat.rs` `scheduler/engine.rs` `compact.rs` `state/{memory_stack,mod,app_state,background}.rs` `scheduler/mod.rs` `organize/assemble.rs` | 刀④a：占用收成 `occupancy` 一个出处 + `tree` 参数整链摘除 + 装配口 `assemble`（**`events.rs` / `runner/projection.rs` 不动**：载荷没加字段，理由见上面计划第 4 条那条 ⚠） |
| `desktop/src/{components/ContextProgressBar.vue,components/chat/UsageRing.vue,components/chat/ContextUsagePopover.vue,stores/chat/{projection,state}.ts,types/events-agent.ts,lib/api/channels.ts}` | 刀④b：分子不动（后端已换源，字段形状一字未改）+ 扣减链一行（文案中英成对） |
| `desktop/src-tauri/src/settings/context.rs` + `components/settings/ContextSection.vue` + `locales/{zh-CN,en}/settings.ts` | 刀④（若裁定 ③ 选可调项则多一格字段）：说明文案引用扣减链 |
| `crates/kernel-agent/tests/prompt_layering.rs` `memory_inject/tests/*` `desktop/tests/*` `scripts/check-single-source.mjs` | 全程：形状用例订正 + 新判据（标签等值 / 占用等值 / 退役名清单）+ 各配变异 |

## 风险与回滚

- 风险①：**倒退窗口**（ADR-0007 风险③ 点名的就是 S1–S3 间隙）。刀① 一摘
  `<persistent_memory>` 正文，而"按需读"要靠 `memory_read` / 文件工具真跑通；
  树工具到 S4 才退役 ⇒ 期间指引与实际能力必须**一致**（AC-S3-5 的判据为这一格存在）。
- 风险②：刀② 把 mtime 塞进缓存键 = 砸前缀缓存的通路。命中态必须有用例反证
  （不改文件 ⇒ 逐字节相同），否则等于把 S2 刀③ 花力气守的"被清前缀稳定"从另一头漏掉。
- 风险③：刀④ 改分子 = **改界面读数口径**，不是修 bug。开工前要先读到消费端
  （`ContextProgressBar` 与 `UsageRing` 两处 + 重放链 `runner/projection.rs:106-120`），
  并如实写成"旧行为是只吃上报数"，不许说成"改成拒绝"。
- 回滚：每刀一提交；刀①②③ 有现成门（`memory.enabled=false` / `top_k=0` ⇒ 整块缺席，
  `mod.rs:215-217`），刀④ 是读数口径，回滚 = revert 该提交（不动数据）。
- 挂账：本线不销 ⓝ/ⓞ；ⓖ/ⓗ 仍等你批形态；specs/09 的「主层与 spill 的**真机**长会话一轮」
  与「spill 目录清理通路」两欠与本线无关，仍挂在 roadmap 表上。
- **新欠账 ⓠ（刀③b 查实，刀① 留下）**：`MemorySource::recall` 在生产面已无调用点，而它是
  注入埋点 `recall_quiet → memory_inject_log` 的唯一挂点 ⇒ **L0 那三个指标自刀① 起停止增长**
  （管理页那块读数停在历史值）。三条出路（ⓠa 给索引段埋点并重定口径 / ⓠb 观测面板整块退役 /
  ⓠc 保持现状并在文案上如实写"本版起停止累计"）在刀③b 施工记录末尾那条里写清了，
  **等决策人挑一条**；本刀不动它的形状，`SS29c` 钉着它还在。
